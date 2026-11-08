import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import * as anchor from '@coral-xyz/anchor';
import { assertSolanaNetwork } from '../clients/js/src/network-guard.js';
import { confirmTransaction } from './transaction-approval.js';

const value = process.argv[2];
if (value !== 'true' && value !== 'false') throw new Error('Usage: npm run pause -- true|false');
const root = resolve(new URL('.', import.meta.url).pathname, '..');
const config = JSON.parse(await readFile(resolve(root, 'config/devnet.json'), 'utf8')) as { cluster: 'devnet' | 'localnet'; rpcUrl: string; vault: string };
await assertSolanaNetwork(config.rpcUrl, config.cluster);
const owner = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(await readFile(resolve(root, 'keys/maria-keypair.json'), 'utf8'))));
const provider = new anchor.AnchorProvider(new anchor.web3.Connection(config.rpcUrl, 'confirmed'), new anchor.Wallet(owner), { commitment: 'confirmed' });
const program = new anchor.Program(JSON.parse(await readFile(resolve(root, 'target/idl/agent_vault.json'), 'utf8')) as anchor.Idl, provider);
const transaction = await program.methods.setPaused(value === 'true').accounts({ vault: config.vault, authority: owner.publicKey }).transaction();
const simulation = await provider.simulate(transaction, [owner]);
await confirmTransaction(transaction, {
  action: value === 'true' ? 'Pause Agent Vault' : 'Resume Agent Vault',
  cluster: config.cluster, feePayer: owner.publicKey.toBase58(),
  recipients: [{ address: config.vault }],
});
const signature = await provider.sendAndConfirm(transaction, [owner]);
console.log(JSON.stringify({ paused: value === 'true', signature }, null, 2));
