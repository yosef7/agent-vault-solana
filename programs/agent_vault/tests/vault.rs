use {
    anchor_lang::{
        prelude::{Clock, Pubkey},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    solana_account::Account,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_program_option::COption,
    solana_program_pack::Pack,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    spl_associated_token_account_interface::address::get_associated_token_address,
    spl_token_interface::{
        state::{Account as SplAccount, AccountState, Mint as SplMint},
        ID as TOKEN_PROGRAM_ID,
    },
};

fn send(
    svm: &mut LiteSVM,
    payer: &Keypair,
    instruction: anchor_lang::solana_program::instruction::Instruction,
) -> Result<(), String> {
    svm.expire_blockhash();
    let message = Message::new_with_blockhash(
        &[instruction],
        Some(&payer.pubkey()),
        &svm.latest_blockhash(),
    );
    let transaction = VersionedTransaction::try_new(VersionedMessage::Legacy(message), &[payer])
        .map_err(|error| error.to_string())?;
    svm.send_transaction(transaction)
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

fn token_account(mint: Pubkey, owner: Pubkey, amount: u64) -> Account {
    let token = SplAccount {
        mint,
        owner,
        amount,
        delegate: COption::None,
        state: AccountState::Initialized,
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    };
    let mut data = vec![0; SplAccount::LEN];
    SplAccount::pack(token, &mut data).unwrap();
    Account {
        lamports: 2_000_000,
        data,
        owner: TOKEN_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    }
}

fn token_amount(svm: &LiteSVM, address: &Pubkey) -> u64 {
    let account = svm.get_account(address).unwrap();
    SplAccount::unpack(&account.data).unwrap().amount
}

struct ReceiptFixture {
    svm: LiteSVM,
    owner: Keypair,
    agent: Keypair,
    impostor: Keypair,
    vault: Pubkey,
    vault_tokens: Pubkey,
    merchant_tokens: Pubkey,
    merchant_tokens_alt: Pubkey,
    wrong_tokens: Pubkey,
    mint: Pubkey,
}

fn receipt_fixture(daily_limit: u64, approval_threshold: u64) -> ReceiptFixture {
    let mut svm = LiteSVM::new();
    svm.add_program(
        agent_vault::id(),
        include_bytes!(concat!(
            env!("CARGO_TARGET_TMPDIR"),
            "/../deploy/agent_vault.so"
        )),
    )
    .unwrap();
    let owner = Keypair::new();
    let agent = Keypair::new();
    let impostor = Keypair::new();
    let merchant = Keypair::new();
    let wrong_merchant = Keypair::new();
    let mint = Keypair::new();
    for signer in [&owner, &agent, &impostor] {
        svm.airdrop(&signer.pubkey(), 1_000_000_000).unwrap();
    }

    let mut mint_data = vec![0; SplMint::LEN];
    SplMint::pack(
        SplMint {
            mint_authority: COption::Some(owner.pubkey()),
            supply: 1_000,
            decimals: 6,
            is_initialized: true,
            freeze_authority: COption::None,
        },
        &mut mint_data,
    )
    .unwrap();
    svm.set_account(
        mint.pubkey(),
        Account {
            lamports: 2_000_000,
            data: mint_data,
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (vault, _) =
        Pubkey::find_program_address(&[b"vault", owner.pubkey().as_ref()], &agent_vault::id());
    let vault_tokens = get_associated_token_address(&vault, &mint.pubkey());
    let merchant_tokens = get_associated_token_address(&merchant.pubkey(), &mint.pubkey());
    let merchant_tokens_alt = Pubkey::new_unique();
    let wrong_tokens = get_associated_token_address(&wrong_merchant.pubkey(), &mint.pubkey());
    let initialize = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        agent_vault::id(),
        &agent_vault::instruction::Initialize {
            agent: agent.pubkey(),
            merchants: [
                merchant.pubkey(),
                Pubkey::default(),
                Pubkey::default(),
                Pubkey::default(),
                Pubkey::default(),
            ],
            merchant_count: 1,
            daily_limit,
            approval_threshold,
        }
        .data(),
        agent_vault::accounts::Initialize {
            authority: owner.pubkey(),
            vault,
            vault_tokens,
            mint: mint.pubkey(),
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &owner, initialize).unwrap();
    svm.set_account(vault_tokens, token_account(mint.pubkey(), vault, 200))
        .unwrap();
    svm.set_account(
        merchant_tokens,
        token_account(mint.pubkey(), merchant.pubkey(), 0),
    )
    .unwrap();
    svm.set_account(
        merchant_tokens_alt,
        token_account(mint.pubkey(), merchant.pubkey(), 0),
    )
    .unwrap();
    svm.set_account(
        wrong_tokens,
        token_account(mint.pubkey(), wrong_merchant.pubkey(), 0),
    )
    .unwrap();

    ReceiptFixture {
        svm,
        owner,
        agent,
        impostor,
        vault,
        vault_tokens,
        merchant_tokens,
        merchant_tokens_alt,
        wrong_tokens,
        mint: mint.pubkey(),
    }
}

fn request_ix(
    fixture: &ReceiptFixture,
    request_hash: [u8; 32],
    merchant_tokens: Pubkey,
    agent: Pubkey,
    amount: u64,
) -> anchor_lang::solana_program::instruction::Instruction {
    let (receipt, _) = Pubkey::find_program_address(
        &[b"receipt", fixture.vault.as_ref(), request_hash.as_ref()],
        &agent_vault::id(),
    );
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        agent_vault::id(),
        &agent_vault::instruction::RequestPayment {
            request_hash,
            amount,
        }
        .data(),
        agent_vault::accounts::RequestPayment {
            vault: fixture.vault,
            receipt,
            vault_tokens: fixture.vault_tokens,
            merchant_tokens,
            mint: fixture.mint,
            agent,
            token_program: TOKEN_PROGRAM_ID,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn receipt_state(svm: &LiteSVM, address: &Pubkey) -> agent_vault::Receipt {
    let account = svm.get_account(address).unwrap();
    let mut bytes: &[u8] = &account.data;
    agent_vault::Receipt::try_deserialize(&mut bytes).unwrap()
}

fn receipt_address(vault: Pubkey, request_hash: [u8; 32]) -> Pubkey {
    Pubkey::find_program_address(
        &[b"receipt", vault.as_ref(), request_hash.as_ref()],
        &agent_vault::id(),
    )
    .0
}

fn agent_pay_ix(
    fixture: &ReceiptFixture,
    merchant_tokens: Pubkey,
    agent: Pubkey,
    amount: u64,
) -> anchor_lang::solana_program::instruction::Instruction {
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        agent_vault::id(),
        &agent_vault::instruction::AgentPay { amount }.data(),
        agent_vault::accounts::AgentPay {
            vault: fixture.vault,
            vault_tokens: fixture.vault_tokens,
            merchant_tokens,
            mint: fixture.mint,
            agent,
            token_program: TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    )
}

// Regression: agent_pay must not settle amounts above approval_threshold
// without the owner co-sign that request_payment + approve_payment require.
#[test]
fn agent_pay_rejects_amount_above_approval_threshold() {
    let mut fixture = receipt_fixture(100, 10);

    let over = agent_pay_ix(
        &fixture,
        fixture.merchant_tokens,
        fixture.agent.pubkey(),
        11,
    );
    let error = send(&mut fixture.svm, &fixture.agent, over).unwrap_err();
    // VaultError::ApprovalRequired is the 14th variant: 6000 + 13.
    assert!(error.contains("Custom(6013)"), "unexpected error: {error}");

    let full_cap = agent_pay_ix(
        &fixture,
        fixture.merchant_tokens,
        fixture.agent.pubkey(),
        100,
    );
    assert!(send(&mut fixture.svm, &fixture.agent, full_cap).is_err());
    assert_eq!(token_amount(&fixture.svm, &fixture.merchant_tokens), 0);
    assert_eq!(token_amount(&fixture.svm, &fixture.vault_tokens), 200);

    let vault_account = fixture.svm.get_account(&fixture.vault).unwrap();
    let mut bytes: &[u8] = &vault_account.data;
    let state = agent_vault::Vault::try_deserialize(&mut bytes).unwrap();
    assert_eq!(state.spent_today, 0);
}

#[test]
fn agent_pay_allows_amount_at_approval_threshold() {
    let mut fixture = receipt_fixture(100, 10);

    let at_threshold = agent_pay_ix(
        &fixture,
        fixture.merchant_tokens,
        fixture.agent.pubkey(),
        10,
    );
    send(&mut fixture.svm, &fixture.agent, at_threshold).unwrap();
    assert_eq!(token_amount(&fixture.svm, &fixture.merchant_tokens), 10);
    assert_eq!(token_amount(&fixture.svm, &fixture.vault_tokens), 190);
}

#[test]
fn vault_enforces_agent_merchant_limit_pause_and_owner_withdrawal() {
    let mut svm = LiteSVM::new();
    svm.add_program(
        agent_vault::id(),
        include_bytes!(concat!(
            env!("CARGO_TARGET_TMPDIR"),
            "/../deploy/agent_vault.so"
        )),
    )
    .unwrap();
    let owner = Keypair::new();
    let agent = Keypair::new();
    let impostor = Keypair::new();
    let merchant = Keypair::new();
    let wrong_merchant = Keypair::new();
    let mint = Keypair::new();
    for signer in [&owner, &agent, &impostor] {
        svm.airdrop(&signer.pubkey(), 1_000_000_000).unwrap();
    }
    let mut mint_data = vec![0; SplMint::LEN];
    SplMint::pack(
        SplMint {
            mint_authority: COption::Some(owner.pubkey()),
            supply: 1_000,
            decimals: 6,
            is_initialized: true,
            freeze_authority: COption::None,
        },
        &mut mint_data,
    )
    .unwrap();
    svm.set_account(
        mint.pubkey(),
        Account {
            lamports: 2_000_000,
            data: mint_data,
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (vault, _) =
        Pubkey::find_program_address(&[b"vault", owner.pubkey().as_ref()], &agent_vault::id());
    let vault_tokens = get_associated_token_address(&vault, &mint.pubkey());
    let merchant_tokens = get_associated_token_address(&merchant.pubkey(), &mint.pubkey());
    let wrong_tokens = get_associated_token_address(&wrong_merchant.pubkey(), &mint.pubkey());
    let owner_tokens = get_associated_token_address(&owner.pubkey(), &mint.pubkey());
    let initialize = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        agent_vault::id(),
        &agent_vault::instruction::Initialize {
            agent: agent.pubkey(),
            merchants: [
                merchant.pubkey(),
                Pubkey::default(),
                Pubkey::default(),
                Pubkey::default(),
                Pubkey::default(),
            ],
            merchant_count: 1,
            daily_limit: 100,
            approval_threshold: 100,
        }
        .data(),
        agent_vault::accounts::Initialize {
            authority: owner.pubkey(),
            vault,
            vault_tokens,
            mint: mint.pubkey(),
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &owner, initialize).unwrap();
    svm.set_account(vault_tokens, token_account(mint.pubkey(), vault, 200))
        .unwrap();
    svm.set_account(
        merchant_tokens,
        token_account(mint.pubkey(), merchant.pubkey(), 0),
    )
    .unwrap();
    svm.set_account(
        wrong_tokens,
        token_account(mint.pubkey(), wrong_merchant.pubkey(), 0),
    )
    .unwrap();
    svm.set_account(
        owner_tokens,
        token_account(mint.pubkey(), owner.pubkey(), 0),
    )
    .unwrap();

    let pay = |agent_key: Pubkey, destination: Pubkey, amount: u64| {
        anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
            agent_vault::id(),
            &agent_vault::instruction::AgentPay { amount }.data(),
            agent_vault::accounts::AgentPay {
                vault,
                vault_tokens,
                merchant_tokens: destination,
                mint: mint.pubkey(),
                agent: agent_key,
                token_program: TOKEN_PROGRAM_ID,
            }
            .to_account_metas(None),
        )
    };
    assert!(send(
        &mut svm,
        &impostor,
        pay(impostor.pubkey(), merchant_tokens, 1)
    )
    .is_err());
    assert!(send(&mut svm, &agent, pay(agent.pubkey(), wrong_tokens, 1)).is_err());
    send(&mut svm, &agent, pay(agent.pubkey(), merchant_tokens, 40)).unwrap();
    assert_eq!(token_amount(&svm, &merchant_tokens), 40);
    assert!(send(&mut svm, &agent, pay(agent.pubkey(), merchant_tokens, 61)).is_err());
    assert_eq!(token_amount(&svm, &merchant_tokens), 40);

    let pause = |authority: Pubkey, paused| {
        anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
            agent_vault::id(),
            &agent_vault::instruction::SetPaused { paused }.data(),
            agent_vault::accounts::OwnerControl { vault, authority }.to_account_metas(None),
        )
    };
    assert!(send(&mut svm, &impostor, pause(impostor.pubkey(), true)).is_err());
    send(&mut svm, &owner, pause(owner.pubkey(), true)).unwrap();
    assert!(send(&mut svm, &agent, pay(agent.pubkey(), merchant_tokens, 1)).is_err());
    send(&mut svm, &owner, pause(owner.pubkey(), false)).unwrap();

    let withdraw = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        agent_vault::id(),
        &agent_vault::instruction::OwnerWithdraw { amount: 20 }.data(),
        agent_vault::accounts::OwnerWithdraw {
            vault,
            vault_tokens,
            owner_tokens,
            mint: mint.pubkey(),
            authority: owner.pubkey(),
            token_program: TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &owner, withdraw).unwrap();
    assert_eq!(token_amount(&svm, &owner_tokens), 20);
    assert_eq!(token_amount(&svm, &vault_tokens), 140);

    let vault_account = svm.get_account(&vault).unwrap();
    let mut bytes: &[u8] = &vault_account.data;
    let state = agent_vault::Vault::try_deserialize(&mut bytes).unwrap();
    assert_eq!(state.spent_today, 40);
    assert!(!state.paused);

    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp += 86_400;
    svm.set_sysvar(&clock);
    send(&mut svm, &agent, pay(agent.pubkey(), merchant_tokens, 100)).unwrap();
    assert_eq!(token_amount(&svm, &merchant_tokens), 140);
    assert_eq!(token_amount(&svm, &vault_tokens), 40);
    let vault_account = svm.get_account(&vault).unwrap();
    let mut bytes: &[u8] = &vault_account.data;
    let state = agent_vault::Vault::try_deserialize(&mut bytes).unwrap();
    assert_eq!(state.spent_today, 100);
}

#[test]
fn request_payment_paid_creates_receipt_and_transfers() {
    let mut fixture = receipt_fixture(100, 10);
    let request_hash = [1; 32];
    let receipt = receipt_address(fixture.vault, request_hash);
    let instruction = request_ix(
        &fixture,
        request_hash,
        fixture.merchant_tokens,
        fixture.agent.pubkey(),
        10,
    );

    send(&mut fixture.svm, &fixture.agent, instruction).unwrap();

    let state = receipt_state(&fixture.svm, &receipt);
    assert!(matches!(state.status, agent_vault::ReceiptStatus::Paid));
    assert!(matches!(state.reason, agent_vault::RejectReason::None));
    assert_eq!(state.amount, 10);
    assert_eq!(token_amount(&fixture.svm, &fixture.merchant_tokens), 10);
}

#[test]
fn request_payment_pending_can_be_approved() {
    let mut fixture = receipt_fixture(100, 10);
    let request_hash = [2; 32];
    let receipt = receipt_address(fixture.vault, request_hash);
    let request = request_ix(
        &fixture,
        request_hash,
        fixture.merchant_tokens,
        fixture.agent.pubkey(),
        11,
    );
    send(&mut fixture.svm, &fixture.agent, request).unwrap();
    assert!(matches!(
        receipt_state(&fixture.svm, &receipt).status,
        agent_vault::ReceiptStatus::Pending
    ));
    assert_eq!(token_amount(&fixture.svm, &fixture.merchant_tokens), 0);

    let approve = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        agent_vault::id(),
        &agent_vault::instruction::ApprovePayment.data(),
        agent_vault::accounts::ApprovePayment {
            vault: fixture.vault,
            receipt,
            vault_tokens: fixture.vault_tokens,
            merchant_tokens: fixture.merchant_tokens,
            mint: fixture.mint,
            authority: fixture.owner.pubkey(),
            token_program: TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    send(&mut fixture.svm, &fixture.owner, approve).unwrap();

    assert!(matches!(
        receipt_state(&fixture.svm, &receipt).status,
        agent_vault::ReceiptStatus::Paid
    ));
    assert_eq!(token_amount(&fixture.svm, &fixture.merchant_tokens), 11);
}

#[test]
fn approve_payment_rejects_alternate_token_account_for_same_merchant() {
    let mut fixture = receipt_fixture(100, 10);
    let request_hash = [10; 32];
    let receipt = receipt_address(fixture.vault, request_hash);
    let request = request_ix(
        &fixture,
        request_hash,
        fixture.merchant_tokens,
        fixture.agent.pubkey(),
        11,
    );
    send(&mut fixture.svm, &fixture.agent, request).unwrap();

    let approve_alternate = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        agent_vault::id(),
        &agent_vault::instruction::ApprovePayment.data(),
        agent_vault::accounts::ApprovePayment {
            vault: fixture.vault,
            receipt,
            vault_tokens: fixture.vault_tokens,
            merchant_tokens: fixture.merchant_tokens_alt,
            mint: fixture.mint,
            authority: fixture.owner.pubkey(),
            token_program: TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    assert!(send(&mut fixture.svm, &fixture.owner, approve_alternate).is_err());
    assert_eq!(token_amount(&fixture.svm, &fixture.merchant_tokens), 0);
    assert_eq!(token_amount(&fixture.svm, &fixture.merchant_tokens_alt), 0);
    assert!(matches!(
        receipt_state(&fixture.svm, &receipt).status,
        agent_vault::ReceiptStatus::Pending
    ));
}

#[test]
fn request_payment_pending_can_be_rejected() {
    let mut fixture = receipt_fixture(100, 10);
    let request_hash = [3; 32];
    let receipt = receipt_address(fixture.vault, request_hash);
    let request = request_ix(
        &fixture,
        request_hash,
        fixture.merchant_tokens,
        fixture.agent.pubkey(),
        11,
    );
    send(&mut fixture.svm, &fixture.agent, request).unwrap();

    let reject = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        agent_vault::id(),
        &agent_vault::instruction::RejectPayment.data(),
        agent_vault::accounts::RejectPayment {
            vault: fixture.vault,
            receipt,
            authority: fixture.owner.pubkey(),
        }
        .to_account_metas(None),
    );
    send(&mut fixture.svm, &fixture.owner, reject).unwrap();

    let state = receipt_state(&fixture.svm, &receipt);
    assert!(matches!(state.status, agent_vault::ReceiptStatus::Rejected));
    assert!(matches!(
        state.reason,
        agent_vault::RejectReason::OwnerRejected
    ));
    assert_eq!(token_amount(&fixture.svm, &fixture.merchant_tokens), 0);
}

#[test]
fn request_payment_rejects_unlisted_merchant_without_transfer() {
    let mut fixture = receipt_fixture(100, 100);
    let request_hash = [4; 32];
    let receipt = receipt_address(fixture.vault, request_hash);

    let request = request_ix(
        &fixture,
        request_hash,
        fixture.wrong_tokens,
        fixture.agent.pubkey(),
        10,
    );
    send(&mut fixture.svm, &fixture.agent, request).unwrap();

    let state = receipt_state(&fixture.svm, &receipt);
    assert!(matches!(state.status, agent_vault::ReceiptStatus::Rejected));
    assert!(matches!(
        state.reason,
        agent_vault::RejectReason::MerchantNotAllowed
    ));
    assert_eq!(token_amount(&fixture.svm, &fixture.wrong_tokens), 0);
}

#[test]
fn request_payment_rejects_daily_limit_without_transfer() {
    let mut fixture = receipt_fixture(10, 100);
    let first_hash = [5; 32];
    let second_hash = [6; 32];
    let first_request = request_ix(
        &fixture,
        first_hash,
        fixture.merchant_tokens,
        fixture.agent.pubkey(),
        10,
    );
    send(&mut fixture.svm, &fixture.agent, first_request).unwrap();
    let second_request = request_ix(
        &fixture,
        second_hash,
        fixture.merchant_tokens,
        fixture.agent.pubkey(),
        1,
    );
    send(&mut fixture.svm, &fixture.agent, second_request).unwrap();

    let state = receipt_state(&fixture.svm, &receipt_address(fixture.vault, second_hash));
    assert!(matches!(state.status, agent_vault::ReceiptStatus::Rejected));
    assert!(matches!(
        state.reason,
        agent_vault::RejectReason::DailyLimit
    ));
    assert_eq!(token_amount(&fixture.svm, &fixture.merchant_tokens), 10);
}

#[test]
fn request_payment_rejects_when_vault_is_paused() {
    let mut fixture = receipt_fixture(100, 100);
    let pause = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        agent_vault::id(),
        &agent_vault::instruction::SetPaused { paused: true }.data(),
        agent_vault::accounts::OwnerControl {
            vault: fixture.vault,
            authority: fixture.owner.pubkey(),
        }
        .to_account_metas(None),
    );
    send(&mut fixture.svm, &fixture.owner, pause).unwrap();

    let request_hash = [7; 32];
    let receipt = receipt_address(fixture.vault, request_hash);
    let request = request_ix(
        &fixture,
        request_hash,
        fixture.merchant_tokens,
        fixture.agent.pubkey(),
        10,
    );
    send(&mut fixture.svm, &fixture.agent, request).unwrap();

    let state = receipt_state(&fixture.svm, &receipt);
    assert!(matches!(state.status, agent_vault::ReceiptStatus::Rejected));
    assert!(matches!(state.reason, agent_vault::RejectReason::Paused));
    assert_eq!(token_amount(&fixture.svm, &fixture.merchant_tokens), 0);
}

#[test]
fn request_payment_rejects_repeated_request_hash() {
    let mut fixture = receipt_fixture(100, 100);
    let request_hash = [8; 32];
    let instruction = request_ix(
        &fixture,
        request_hash,
        fixture.merchant_tokens,
        fixture.agent.pubkey(),
        10,
    );
    send(&mut fixture.svm, &fixture.agent, instruction).unwrap();

    let repeated_request = request_ix(
        &fixture,
        request_hash,
        fixture.merchant_tokens,
        fixture.agent.pubkey(),
        10,
    );
    assert!(send(&mut fixture.svm, &fixture.agent, repeated_request).is_err());
    assert_eq!(token_amount(&fixture.svm, &fixture.merchant_tokens), 10);
}

#[test]
fn request_payment_rejects_wrong_signer() {
    let mut fixture = receipt_fixture(100, 100);
    let request_hash = [9; 32];

    let request = request_ix(
        &fixture,
        request_hash,
        fixture.merchant_tokens,
        fixture.impostor.pubkey(),
        10,
    );
    assert!(send(&mut fixture.svm, &fixture.impostor, request).is_err());
    assert!(fixture
        .svm
        .get_account(&receipt_address(fixture.vault, request_hash))
        .is_none());
    assert_eq!(token_amount(&fixture.svm, &fixture.merchant_tokens), 0);
}
