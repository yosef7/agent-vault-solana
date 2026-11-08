# Security policy

Agent Vault is a hackathon prototype. It runs on Solana devnet with a test token and has not been audited. Do not use it with real funds.

## Reporting a vulnerability

Please report security issues privately. Do not open a public issue.

- Use GitHub's **Report a vulnerability** button (private vulnerability reporting) on this repository, or
- email the maintainer at the address on the commits.

Include the commit, the affected instruction or file, and the steps to reproduce. Source-level findings are welcome. Please do not send transactions against vaults you do not own.

You should get a reply within 7 days. There is no paid bounty; reporters are credited in the fix unless they ask not to be.

## Fixed issues

| Date | Issue | Fix |
| --- | --- | --- |
| 2026-11-08 | `agent_pay` did not check `approval_threshold`, so the agent could pay an allowlisted merchant up to the daily cap without the owner's co-sign. | `agent_pay` now rejects amounts above the threshold with `ApprovalRequired`; regression tests in `programs/agent_vault/tests/vault.rs`. |
