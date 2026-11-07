import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import * as anchor from '@coral-xyz/anchor';
import { PublicKey } from '@solana/web3.js';
import { TOKEN_PROGRAM_ID } from '@solana/spl-token';
import { assertSolanaNetwork } from '../clients/js/src/network-guard.js';

const amount = process.argv[2];
if (!amount || !/^[1-9]\d*$/.test(amount)) throw new Error('Usage: npm run withdraw -- <base units>');
const root = resolve(new URL('.', import.meta.url).pathname, '..');
const config = JSON.parse(await readFile(resolve(root, 'config/devnet.json'), 'utf8')) as { cluster: 'devnet' | 'localnet'; rpcUrl: string; vault: string; vaultTokens: string; mint: string };
await assertSolanaNetwork(config.rpcUrl, config.cluster);
const owner = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(await readFile(resolve(root, 'keys/maria-keypair.json'), 'utf8'))));
const provider = new anchor.AnchorProvider(new anchor.web3.Connection(config.rpcUrl, 'confirmed'), new anchor.Wallet(owner), { commitment: 'confirmed' });
const program = new anchor.Program(JSON.parse(await readFile(resolve(root, 'target/idl/agent_vault.json'), 'utf8')) as anchor.Idl, provider);
const ownerTokens = await (await import('@solana/spl-token')).getAssociatedTokenAddress(new PublicKey(config.mint), owner.publicKey);
const transaction = await program.methods.ownerWithdraw(new anchor.BN(amount)).accounts({
  vault: config.vault, vaultTokens: config.vaultTokens, ownerTokens, mint: config.mint, authority: owner.publicKey, tokenProgram: TOKEN_PROGRAM_ID,
}).transaction();
await provider.simulate(transaction, [owner]);
const signature = await provider.sendAndConfirm(transaction, [owner]);
console.log(JSON.stringify({ amount, signature }, null, 2));
