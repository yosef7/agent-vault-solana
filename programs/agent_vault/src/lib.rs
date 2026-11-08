use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Mint, Token, TokenAccount, TransferChecked},
};

declare_id!("41pzYtwB4MJV2c7uyhEqSUYc3awtekzsPiAGLiPr4Fvi");

const VAULT_SEED: &[u8] = b"vault";
const RECEIPT_SEED: &[u8] = b"receipt";
const MAX_MERCHANTS: usize = 5;
const SECONDS_PER_DAY: i64 = 86_400;

#[program]
pub mod agent_vault {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        agent: Pubkey,
        merchants: [Pubkey; MAX_MERCHANTS],
        merchant_count: u8,
        daily_limit: u64,
        approval_threshold: u64,
    ) -> Result<()> {
        require!(daily_limit > 0, VaultError::InvalidAmount);
        require!(agent != Pubkey::default(), VaultError::InvalidAgent);
        require!(
            merchant_count > 0 && merchant_count <= MAX_MERCHANTS as u8,
            VaultError::InvalidMerchantCount
        );
        require!(
            merchants[..merchant_count as usize]
                .iter()
                .all(|merchant| *merchant != Pubkey::default()),
            VaultError::InvalidMerchant
        );

        let vault = &mut ctx.accounts.vault;
        vault.authority = ctx.accounts.authority.key();
        vault.agent = agent;
        vault.merchants = merchants;
        vault.merchant_count = merchant_count;
        vault.mint = ctx.accounts.mint.key();
        vault.daily_limit = daily_limit;
        vault.approval_threshold = approval_threshold;
        vault.spent_today = 0;
        vault.day_index = current_day(Clock::get()?.unix_timestamp)?;
        vault.paused = false;
        vault.bump = ctx.bumps.vault;
        Ok(())
    }

    pub fn agent_pay(ctx: Context<AgentPay>, amount: u64) -> Result<()> {
        require!(amount > 0, VaultError::InvalidAmount);
        let vault = &mut ctx.accounts.vault;
        require!(!vault.paused, VaultError::Paused);
        require_keys_eq!(
            ctx.accounts.agent.key(),
            vault.agent,
            VaultError::UnauthorizedAgent
        );
        require!(
            is_allowed_merchant(vault, ctx.accounts.merchant_tokens.owner),
            VaultError::WrongMerchant
        );
        // Same rule as request_payment: anything above the owner's approval
        // threshold must go through a Pending receipt and an owner co-sign.
        require!(
            amount <= vault.approval_threshold,
            VaultError::ApprovalRequired
        );
        let day = current_day(Clock::get()?.unix_timestamp)?;
        let new_spent = spending_after_payment(
            vault.day_index,
            vault.spent_today,
            day,
            amount,
            vault.daily_limit,
        )?;

        let authority_key = vault.authority;
        let bump = [vault.bump];
        let signer_seeds: &[&[&[u8]]] = &[&[VAULT_SEED, authority_key.as_ref(), &bump]];
        let cpi_accounts = TransferChecked {
            from: ctx.accounts.vault_tokens.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.merchant_tokens.to_account_info(),
            authority: vault.to_account_info(),
        };
        token::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                cpi_accounts,
                signer_seeds,
            ),
            amount,
            ctx.accounts.mint.decimals,
        )?;
        vault.day_index = day;
        vault.spent_today = new_spent;
        emit!(PaymentMade {
            authority: vault.authority,
            merchant: ctx.accounts.merchant_tokens.owner,
            amount,
            day_index: day,
        });
        Ok(())
    }

    pub fn request_payment(
        ctx: Context<RequestPayment>,
        request_hash: [u8; 32],
        amount: u64,
    ) -> Result<()> {
        require!(amount > 0, VaultError::InvalidAmount);
        let now = Clock::get()?.unix_timestamp;
        let day = current_day(now)?;
        let vault = &mut ctx.accounts.vault;
        require_keys_eq!(
            ctx.accounts.agent.key(),
            vault.agent,
            VaultError::UnauthorizedAgent
        );

        let merchant = ctx.accounts.merchant_tokens.owner;
        let (status, reason, should_pay) = if vault.paused {
            (ReceiptStatus::Rejected, RejectReason::Paused, false)
        } else if !is_allowed_merchant(vault, merchant) {
            (
                ReceiptStatus::Rejected,
                RejectReason::MerchantNotAllowed,
                false,
            )
        } else if amount > vault.approval_threshold {
            (ReceiptStatus::Pending, RejectReason::None, false)
        } else {
            require!(day >= vault.day_index, VaultError::InvalidClock);
            let prior_spending = if day == vault.day_index {
                vault.spent_today
            } else {
                0
            };
            let new_spent = prior_spending
                .checked_add(amount)
                .ok_or(VaultError::Overflow)?;
            if new_spent > vault.daily_limit {
                (ReceiptStatus::Rejected, RejectReason::DailyLimit, false)
            } else {
                vault.day_index = day;
                vault.spent_today = new_spent;
                (ReceiptStatus::Paid, RejectReason::None, true)
            }
        };

        let receipt = &mut ctx.accounts.receipt;
        receipt.vault = vault.key();
        receipt.request_hash = request_hash;
        receipt.merchant = merchant;
        receipt.merchant_tokens = ctx.accounts.merchant_tokens.key();
        receipt.amount = amount;
        receipt.status = status;
        receipt.reason = reason;
        receipt.created_at = now;
        receipt.settled_at = if status == ReceiptStatus::Pending {
            0
        } else {
            now
        };
        receipt.bump = ctx.bumps.receipt;

        if should_pay {
            transfer_from_vault(
                vault,
                &ctx.accounts.vault_tokens,
                &ctx.accounts.merchant_tokens,
                &ctx.accounts.mint,
                &ctx.accounts.token_program,
                amount,
            )?;
        }
        emit!(ReceiptCreated {
            vault: vault.key(),
            receipt: receipt.key(),
            request_hash,
            merchant,
            amount,
            status,
            reason,
        });
        Ok(())
    }

    pub fn approve_payment(ctx: Context<ApprovePayment>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let day = current_day(now)?;
        let vault = &mut ctx.accounts.vault;
        let receipt = &mut ctx.accounts.receipt;
        require!(
            receipt.status == ReceiptStatus::Pending,
            VaultError::ReceiptNotPending
        );
        require!(!vault.paused, VaultError::Paused);
        require_keys_eq!(
            receipt.merchant_tokens,
            ctx.accounts.merchant_tokens.key(),
            VaultError::WrongMerchant
        );
        let new_spent = spending_after_payment(
            vault.day_index,
            vault.spent_today,
            day,
            receipt.amount,
            vault.daily_limit,
        )?;
        transfer_from_vault(
            vault,
            &ctx.accounts.vault_tokens,
            &ctx.accounts.merchant_tokens,
            &ctx.accounts.mint,
            &ctx.accounts.token_program,
            receipt.amount,
        )?;
        vault.day_index = day;
        vault.spent_today = new_spent;
        receipt.status = ReceiptStatus::Paid;
        receipt.reason = RejectReason::None;
        receipt.settled_at = now;
        emit!(ReceiptSettled {
            vault: vault.key(),
            receipt: receipt.key(),
            status: receipt.status,
        });
        Ok(())
    }

    pub fn reject_payment(ctx: Context<RejectPayment>) -> Result<()> {
        let receipt = &mut ctx.accounts.receipt;
        require!(
            receipt.status == ReceiptStatus::Pending,
            VaultError::ReceiptNotPending
        );
        receipt.status = ReceiptStatus::Rejected;
        receipt.reason = RejectReason::OwnerRejected;
        receipt.settled_at = Clock::get()?.unix_timestamp;
        emit!(ReceiptSettled {
            vault: ctx.accounts.vault.key(),
            receipt: receipt.key(),
            status: receipt.status,
        });
        Ok(())
    }

    pub fn set_paused(ctx: Context<OwnerControl>, paused: bool) -> Result<()> {
        ctx.accounts.vault.paused = paused;
        Ok(())
    }

    pub fn owner_withdraw(ctx: Context<OwnerWithdraw>, amount: u64) -> Result<()> {
        require!(amount > 0, VaultError::InvalidAmount);
        let vault = &ctx.accounts.vault;
        let authority_key = vault.authority;
        let bump = [vault.bump];
        let signer_seeds: &[&[&[u8]]] = &[&[VAULT_SEED, authority_key.as_ref(), &bump]];
        let cpi_accounts = TransferChecked {
            from: ctx.accounts.vault_tokens.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.owner_tokens.to_account_info(),
            authority: vault.to_account_info(),
        };
        token::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                cpi_accounts,
                signer_seeds,
            ),
            amount,
            ctx.accounts.mint.decimals,
        )
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = 8 + Vault::INIT_SPACE,
        seeds = [VAULT_SEED, authority.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, Vault>,
    #[account(
        init,
        payer = authority,
        associated_token::mint = mint,
        associated_token::authority = vault
    )]
    pub vault_tokens: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AgentPay<'info> {
    #[account(
        mut,
        seeds = [VAULT_SEED, vault.authority.as_ref()],
        bump = vault.bump,
        has_one = mint @ VaultError::WrongMint
    )]
    pub vault: Account<'info, Vault>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = vault
    )]
    pub vault_tokens: Account<'info, TokenAccount>,
    #[account(
        mut,
        token::mint = mint,
    )]
    pub merchant_tokens: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub agent: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(request_hash: [u8; 32])]
pub struct RequestPayment<'info> {
    #[account(
        mut,
        seeds = [VAULT_SEED, vault.authority.as_ref()],
        bump = vault.bump,
        has_one = mint @ VaultError::WrongMint
    )]
    pub vault: Account<'info, Vault>,
    #[account(
        init,
        payer = agent,
        space = 8 + Receipt::INIT_SPACE,
        seeds = [RECEIPT_SEED, vault.key().as_ref(), request_hash.as_ref()],
        bump
    )]
    pub receipt: Account<'info, Receipt>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = vault
    )]
    pub vault_tokens: Account<'info, TokenAccount>,
    #[account(mut, token::mint = mint)]
    pub merchant_tokens: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub agent: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ApprovePayment<'info> {
    #[account(
        mut,
        seeds = [VAULT_SEED, vault.authority.as_ref()],
        bump = vault.bump,
        has_one = authority @ VaultError::UnauthorizedOwner,
        has_one = mint @ VaultError::WrongMint
    )]
    pub vault: Account<'info, Vault>,
    #[account(
        mut,
        seeds = [RECEIPT_SEED, vault.key().as_ref(), receipt.request_hash.as_ref()],
        bump = receipt.bump,
        has_one = vault
    )]
    pub receipt: Account<'info, Receipt>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = vault
    )]
    pub vault_tokens: Account<'info, TokenAccount>,
    #[account(
        mut,
        token::mint = mint,
        constraint = merchant_tokens.key() == receipt.merchant_tokens @ VaultError::WrongMerchant,
        constraint = merchant_tokens.owner == receipt.merchant @ VaultError::WrongMerchant
    )]
    pub merchant_tokens: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct RejectPayment<'info> {
    #[account(
        seeds = [VAULT_SEED, authority.key().as_ref()],
        bump = vault.bump,
        has_one = authority @ VaultError::UnauthorizedOwner
    )]
    pub vault: Account<'info, Vault>,
    #[account(
        mut,
        seeds = [RECEIPT_SEED, vault.key().as_ref(), receipt.request_hash.as_ref()],
        bump = receipt.bump,
        has_one = vault
    )]
    pub receipt: Account<'info, Receipt>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct OwnerControl<'info> {
    #[account(
        mut,
        seeds = [VAULT_SEED, authority.key().as_ref()],
        bump = vault.bump,
        has_one = authority @ VaultError::UnauthorizedOwner
    )]
    pub vault: Account<'info, Vault>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct OwnerWithdraw<'info> {
    #[account(
        seeds = [VAULT_SEED, authority.key().as_ref()],
        bump = vault.bump,
        has_one = authority @ VaultError::UnauthorizedOwner,
        has_one = mint @ VaultError::WrongMint
    )]
    pub vault: Account<'info, Vault>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = vault
    )]
    pub vault_tokens: Account<'info, TokenAccount>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = authority
    )]
    pub owner_tokens: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[account]
#[derive(InitSpace)]
pub struct Vault {
    pub authority: Pubkey,
    pub agent: Pubkey,
    pub merchants: [Pubkey; MAX_MERCHANTS],
    pub merchant_count: u8,
    pub mint: Pubkey,
    pub daily_limit: u64,
    pub approval_threshold: u64,
    pub spent_today: u64,
    pub day_index: i64,
    pub paused: bool,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Receipt {
    pub vault: Pubkey,
    pub request_hash: [u8; 32],
    pub merchant: Pubkey,
    pub merchant_tokens: Pubkey,
    pub amount: u64,
    pub status: ReceiptStatus,
    pub reason: RejectReason,
    pub created_at: i64,
    pub settled_at: i64,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum ReceiptStatus {
    Paid,
    Pending,
    Rejected,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum RejectReason {
    None,
    Paused,
    MerchantNotAllowed,
    DailyLimit,
    OwnerRejected,
}

#[event]
pub struct PaymentMade {
    pub authority: Pubkey,
    pub merchant: Pubkey,
    pub amount: u64,
    pub day_index: i64,
}

#[event]
pub struct ReceiptCreated {
    pub vault: Pubkey,
    pub receipt: Pubkey,
    pub request_hash: [u8; 32],
    pub merchant: Pubkey,
    pub amount: u64,
    pub status: ReceiptStatus,
    pub reason: RejectReason,
}

#[event]
pub struct ReceiptSettled {
    pub vault: Pubkey,
    pub receipt: Pubkey,
    pub status: ReceiptStatus,
}

fn is_allowed_merchant(vault: &Vault, merchant: Pubkey) -> bool {
    vault.merchants[..vault.merchant_count as usize].contains(&merchant)
}

fn transfer_from_vault<'info>(
    vault: &Account<'info, Vault>,
    vault_tokens: &Account<'info, TokenAccount>,
    merchant_tokens: &Account<'info, TokenAccount>,
    mint: &Account<'info, Mint>,
    token_program: &Program<'info, Token>,
    amount: u64,
) -> Result<()> {
    let authority_key = vault.authority;
    let bump = [vault.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[VAULT_SEED, authority_key.as_ref(), &bump]];
    token::transfer_checked(
        CpiContext::new_with_signer(
            token_program.key(),
            TransferChecked {
                from: vault_tokens.to_account_info(),
                mint: mint.to_account_info(),
                to: merchant_tokens.to_account_info(),
                authority: vault.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        mint.decimals,
    )
}

fn current_day(timestamp: i64) -> Result<i64> {
    require!(timestamp >= 0, VaultError::InvalidClock);
    Ok(timestamp / SECONDS_PER_DAY)
}

fn spending_after_payment(
    previous_day: i64,
    spent_today: u64,
    current_day: i64,
    amount: u64,
    limit: u64,
) -> Result<u64> {
    require!(current_day >= previous_day, VaultError::InvalidClock);
    let prior_spending = if current_day == previous_day {
        spent_today
    } else {
        0
    };
    let total = prior_spending
        .checked_add(amount)
        .ok_or(VaultError::Overflow)?;
    require!(total <= limit, VaultError::DailyLimitExceeded);
    Ok(total)
}

#[error_code]
pub enum VaultError {
    #[msg("Amount and daily limit must be positive")]
    InvalidAmount,
    #[msg("Agent address is invalid")]
    InvalidAgent,
    #[msg("Merchant address is invalid")]
    InvalidMerchant,
    #[msg("Only the configured agent may pay")]
    UnauthorizedAgent,
    #[msg("Only the vault owner may change controls or withdraw")]
    UnauthorizedOwner,
    #[msg("Vault is paused")]
    Paused,
    #[msg("Daily spending limit exceeded")]
    DailyLimitExceeded,
    #[msg("Arithmetic overflow")]
    Overflow,
    #[msg("Clock moved backwards or is invalid")]
    InvalidClock,
    #[msg("Token mint does not match vault configuration")]
    WrongMint,
    #[msg("Token recipient does not match the allowed merchant")]
    WrongMerchant,
    #[msg("The merchant list must contain between one and five addresses")]
    InvalidMerchantCount,
    #[msg("A payment receipt is not pending")]
    ReceiptNotPending,
    #[msg("Amount is above the approval threshold; use request_payment for owner approval")]
    ApprovalRequired,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spending_rejects_over_cap() {
        assert!(spending_after_payment(7, 60, 7, 41, 100).is_err());
        assert_eq!(spending_after_payment(7, 60, 7, 40, 100).unwrap(), 100);
    }

    #[test]
    fn spending_resets_on_new_utc_day() {
        assert_eq!(spending_after_payment(7, 90, 8, 20, 100).unwrap(), 20);
    }

    #[test]
    fn spending_rejects_clock_rollback_and_overflow() {
        assert!(spending_after_payment(8, 10, 7, 1, 100).is_err());
        assert!(spending_after_payment(8, u64::MAX, 8, 1, u64::MAX).is_err());
    }
}
