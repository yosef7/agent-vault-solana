import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import * as anchor from '@coral-xyz/anchor';
import { PublicKey } from '@solana/web3.js';
import { assertSolanaNetwork } from '../clients/js/src/network-guard.js';

const receiptArg = process.argv[2];
if (!receiptArg) throw new Error('Usage: npm run verify -- <receipt PDA>');
const root = resolve(new URL('.', import.meta.url).pathname, '..');
const config = JSON.parse(await readFile(resolve(root, 'config/devnet.json'), 'utf8')) as {
  cluster: 'devnet' | 'localnet'; rpcUrl: string; programId: string; vault: string; agent: string;
};
await assertSolanaNetwork(config.rpcUrl, config.cluster);
const agent = anchor.web3.Keypair.fromSecretKey(
  Uint8Array.from(JSON.parse(await readFile(resolve(root, 'keys/agent-keypair.json'), 'utf8'))),
);
const provider = new anchor.AnchorProvider(new anchor.web3.Connection(config.rpcUrl, 'confirmed'), new anchor.Wallet(agent), { commitment: 'confirmed' });
const program = new anchor.Program(JSON.parse(await readFile(resolve(root, 'target/idl/agent_vault.json'), 'utf8')) as anchor.Idl, provider);
const receipt = new PublicKey(receiptArg);
const data = await (program.account as unknown as {
  receipt: { fetch(address: PublicKey): Promise<{ requestHash: number[] }> };
}).receipt.fetch(receipt);
const hash = Buffer.from(data.requestHash).toString('hex');
const record = JSON.parse(await readFile(resolve(root, 'agent/receipts', `${hash}.json`), 'utf8')) as {
  task: string; api: string; amount: number; reason: string; nonce: string;
};
const canonical = JSON.stringify({ task: record.task, api: record.api, amount: record.amount, reason: record.reason, nonce: record.nonce });
const recalculated = createHash('sha256').update(canonical).digest('hex');
if (recalculated !== hash) {
  console.log('❌ reason does not match on-chain commitment');
  process.exitCode = 1;
} else {
  console.log(`✅ reason matches on-chain commitment (${hash})`);
}
