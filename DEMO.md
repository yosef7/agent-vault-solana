# Agent Vault: demo and submission pack

This document is the H5 rehearsal material for the MLH Solana Hackathon. It
matches the current checkout: the Anchor policy program, LiteSVM tests, devnet
deployment, deterministic AI-agent scenarios, receipt verification, and owner
control scripts are implemented. There is no HTTP API or dashboard in this
submission.

## Verified devnet run

The following receipts and signatures were produced on November 7, 2026 using
only the demo keypairs:

| Scenario | Receipt | Result | Transaction |
| --- | --- | --- | --- |
| 1: allowed weather API | `DorUpLa8YqSS27m6BQfMRBSYYVsrMRC6FR3WjUBRppgm` | Paid | [request](https://explorer.solana.com/tx/5ycCvqzNU18K3RYWkUPpd7QvPBU3nS3hojEzCcu6KEVwFLBdtxHNDVEZ4vDQ4L3VsfaSaNVgNQcEQupP7ts6PzwD?cluster=devnet) |
| 2: premium report | `Da78Q2kQjzDYJwniCM8ZVsaZhQECoVgPHFwu86QkWg1t` | Pending, then approved | [request](https://explorer.solana.com/tx/4chZsBQPD3Ke4s9DssvA8at4xaPTgS3KepsM8aMoVmmxJ2rjEQgqsJnDFPEmMEEopYXPHCX2eXAckAmyYwmCsZa9?cluster=devnet) · [approval](https://explorer.solana.com/tx/3q2SwyW5kCeHr7obZuVhNaUuLcFNroyvUzb3tkCXzrAVC2w2fboSB4RHsM3HoQWv4rw63Xumgn8A1M8jaQgiHb3c?cluster=devnet) |
| 3: unlisted provider | `7rqPFarS8F8Hb9VxZzU6LCtuNTUZbHgyhWuAxCdoU3ZT` | Rejected (`merchantNotAllowed`) | [request](https://explorer.solana.com/tx/4d68LKteqpyhKvo344WjSp6efnMo9nKdABgWTUPRbGR95d3HZgB9ugRXyNxuKAXUkKQCKEKAmzgQgFbiHtKjMf55?cluster=devnet) |
| Pause check | `DmEfrqGNw47yGYA1uouxHzPPzZRsRmP29241NBnz3RLH` | Rejected (`paused`) | [request](https://explorer.solana.com/tx/2qUSxwftTVnXMPCs4qgfnDxMECtjgAgqDhusg6qcW6joJ7neYWZ7AtjHhJckbn4fw3xVQidhQ23oCdR74DLSSydo?cluster=devnet) |

The vault was resumed and its remaining `94,880,000` base units were withdrawn
in [this transaction](https://explorer.solana.com/tx/CaBdrv8zbY5Cv4hkfQYWK8hHt7voqyMhQFof2f6JFMZTPywmF6TPH1h54Ya53Xcdog42CWks9taakaxBA2k8Xmm?cluster=devnet).

## Two-minute recording script

Record in English, or record in Spanish with these English subtitles visible by
default. Keep the terminal and explorer readable; do not show keypair files.

| Time | Screen | Narration |
| --- | --- | --- |
| 0:00-0:15 | README problem statement | "AI agents need to buy small API results, but a free-spending wallet is unsafe and a card is a poor fit for cent-sized requests. Agent Vault is a spending account for agents." |
| 0:15-0:35 | Account diagram | "An owner creates a PDA-controlled vault, selects the token mint, allows up to five merchants, and sets a daily cap and approval threshold. The agent signs requests, but cannot change those rules." |
| 0:35-1:05 | `npm run agent -- --scenario 1`, `2`, `3` | "The agent selects from a catalog. The vault pays an allowed cheap request, holds an expensive one for the owner, and records an unlisted provider as rejected." |
| 1:05-1:25 | `npm run approve -- <pda>` and explorer | "The owner co-signs the pending request; only then do tokens move to the provider." |
| 1:25-1:45 | `npm run verify -- <pda>`; edit receipt JSON and rerun | "The reason is committed on-chain. The original file verifies, while a changed word produces a mismatch." |
| 1:45-2:00 | README limits and roadmap | "This is a hackathon prototype: a six-decimal test token, classic SPL Token, no real x402 facilitator, no audit, and no mainnet. Next we add the HTTP API, receipt closure, and dashboard." |

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

## Reproducible agent commands

```bash
npm run agent -- --scenario 1
npm run agent -- --scenario 2
npm run approve -- <scenario-2-receipt-pda>
npm run agent -- --scenario 3
npm run verify -- <receipt-pda>
```

`npm run pause -- true` enables the emergency stop and `npm run pause -- false`
resumes requests. `npm run withdraw -- <base-units>` returns vault tokens to the
owner. Keep `keys/` and `.env` off screen.

## Submission copy

**Title:** Agent Vault

**Description:**

> Agent Vault is a spending account for AI agents that pay API providers per request on Solana. The owner sets the agent signer, up to five permitted merchants, a daily cap, and an approval threshold. The Anchor program enforces those rules in a PDA-controlled vault and creates an on-chain receipt for every attempt: paid, pending for human approval, or rejected.
>
> The project addresses three AI Track ideas: machine-to-machine payments for cent-sized API calls; security tooling through rules the agent cannot bypass; and verifiable AI because the agent's request reason is committed as `request_hash`, which anyone can recalculate from the canonical request. The current prototype uses a six-decimal test token and is not a full x402 facilitator integration.

**Track rationale:**

- **AI Track:** the agent is the requester and Solana enforces its spending policy; the receipt commits the request hash.
- **Top 3 Overall:** the core product uses Solana accounts, PDA authority, token transfers, and public receipts as product behavior.
- **DeFi Track:** the recording demonstrates PDA custody, a pending approval flow, emergency pause, and owner withdrawal over a project test mint; it is not USDC.

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
