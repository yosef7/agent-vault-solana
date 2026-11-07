# Agent Vault: demo and submission pack

This document is the H5 rehearsal material for the MLH Solana Hackathon. It
matches the current checkout: the Anchor policy program and its LiteSVM tests
are implemented; the API, AI agent, dashboard, and devnet deployment are not
yet part of this checkout.

## Two-minute recording script

Record in English, or record in Spanish with these English subtitles visible by
default. Keep the terminal and explorer readable; do not show keypair files.

| Time | Screen | Narration |
| --- | --- | --- |
| 0:00-0:15 | README problem statement | "AI agents need to buy small API results, but a free-spending wallet is unsafe and a card is a poor fit for cent-sized requests. Agent Vault is a spending account for agents." |
| 0:15-0:35 | Account diagram | "An owner creates a PDA-controlled vault, selects the token mint, allows up to five merchants, and sets a daily cap and approval threshold. The agent signs requests, but cannot change those rules." |
| 0:35-1:05 | `anchor test` output | "The minimum reproducible demo runs offline with LiteSVM. The tests cover authorized payments, wrong agents and merchants, pausing, the daily cap, owner withdrawal, and the UTC reset." |
| 1:05-1:30 | Program source / generated IDL | "Each request gets a Receipt PDA derived from the vault and a request hash. A request can be paid, held for owner approval, or rejected by policy. Rejections are recorded instead of disappearing in a failed transaction." |
| 1:30-1:45 | Localnet or devnet transaction, only if available | "On Solana, the policy and receipt are verifiable by the owner, the provider, and a dashboard without trusting the API operator." |
| 1:45-2:00 | README limits and roadmap | "This is a hackathon prototype: test tokens, classic SPL Token, no real x402 facilitator, no audit, and no mainnet. Next we connect the API and agent, then add a dashboard and recover receipt rent." |

If a devnet transaction is recorded later, replace the 1:30-1:45 line with the
actual confirmed signature and measured latency. Do not use the presentation's
150 ms or sub-cent figures as project measurements.

## Rehearsal checklist

- [ ] `NO_DNA=1 anchor build`
- [ ] `NO_DNA=1 anchor test`
- [ ] If using Localnet, deploy locally and set `SOLANA_LOCALNET_GENESIS_HASH`
- [ ] Confirm the visible network is Localnet or Devnet, never Mainnet
- [ ] Confirm no keypair, `.env`, seed phrase, or personal wallet is visible
- [ ] Show the receipt states and the request hash, not private off-chain reasons
- [ ] Keep the recording between 1:50 and 2:10
- [ ] Make English subtitles visible by default if narration is Spanish
- [ ] State clearly what is implemented and what remains a prototype

## Submission copy

**Title:** Agent Vault

**Description:**

> Agent Vault is a spending account for AI agents that pay API providers per request on Solana. The owner sets the agent signer, up to five permitted merchants, a daily cap, and an approval threshold. The Anchor program enforces those rules in a PDA-controlled vault and creates an on-chain receipt for every attempt: paid, pending for human approval, or rejected.
>
> The project addresses three AI Track ideas: machine-to-machine payments for cent-sized API calls; security tooling through rules the agent cannot bypass; and verifiable AI because the agent's request reason is committed as `request_hash`, which anyone can recalculate from the canonical request. The current prototype uses a six-decimal test token and is not a full x402 facilitator integration.

**Track rationale:**

- **AI Track:** the agent is the requester and Solana enforces its spending policy; the receipt commits the request hash.
- **Top 3 Overall:** the core product uses Solana accounts, PDA authority, token transfers, and public receipts as product behavior.
- **DeFi Track:** consider only if the final recording actually demonstrates stablecoin-style payments; this checkout uses a project test mint, not USDC.

**Links to add before submission:**

- [ ] Public GitHub repository
- [ ] Two-minute video
- [ ] Devnet explorer transaction, only after deployment
- [ ] Screenshot of the implemented interface, if one exists

The event page says the hackathon runs November 6-8, 2026; verify the live
submission form and deadline immediately before clicking Submit Project.

## Flovia reference check

The public [Flovia README](https://github.com/flovia/poc) opens with the
agentic-payments problem and quickly explains its x402/MPP, on-chain signals,
intelligence layer, and frontend flow before listing architecture and setup.
That is useful pacing for this demo: state the problem in the first 15 seconds,
show the product boundary before explaining implementation, and close with
scope and next steps. The linked [Loom video](https://www.loom.com/share/68a7cd7b5ec94036804c0fd21888b02e)
is private, so its timing and closing cannot be assessed; this script does not
claim otherwise.
