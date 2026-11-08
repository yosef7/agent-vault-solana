import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import * as anchor from '@coral-xyz/anchor';
import { PublicKey } from '@solana/web3.js';
import { TOKEN_PROGRAM_ID } from '@solana/spl-token';
import { assertSolanaNetwork } from '../clients/js/src/network-guard.js';
import { confirmTransaction } from './transaction-approval.js';

const receiptArg = process.argv[2];
if (!receiptArg) throw new Error('Usage: npm run approve -- <receipt PDA>');
const root = resolve(new URL('.', import.meta.url).pathname, '..');
const config = JSON.parse(await readFile(resolve(root, 'config/devnet.json'), 'utf8')) as {
  cluster: 'devnet' | 'localnet'; rpcUrl: string; programId: string; vault: string;
  vaultTokens: string; mint: string; maria: string;
};
await assertSolanaNetwork(config.rpcUrl, config.cluster);
const owner = anchor.web3.Keypair.fromSecretKey(
  Uint8Array.from(JSON.parse(await readFile(resolve(root, 'keys/maria-keypair.json'), 'utf8'))),
);
const provider = new anchor.AnchorProvider(new anchor.web3.Connection(config.rpcUrl, 'confirmed'), new anchor.Wallet(owner), { commitment: 'confirmed' });
const program = new anchor.Program(
  JSON.parse(await readFile(resolve(root, 'target/idl/agent_vault.json'), 'utf8')) as anchor.Idl,
  provider,
);
const receipt = new PublicKey(receiptArg);
const data = await (program.account as unknown as {
  receipt: { fetch(address: PublicKey): Promise<{ merchantTokens: PublicKey; amount: anchor.BN; status: unknown }> };
}).receipt.fetch(receipt);
if (!JSON.stringify(data.status).toLowerCase().includes('pending')) {
  throw new Error('Only a pending receipt can be approved.');
}
const transaction = await program.methods.approvePayment().accounts({
  vault: config.vault, receipt, vaultTokens: config.vaultTokens,
  merchantTokens: data.merchantTokens, mint: config.mint, authority: owner.publicKey,
  tokenProgram: TOKEN_PROGRAM_ID,
}).transaction();
const simulation = await provider.simulate(transaction, [owner]);
await confirmTransaction(transaction, {
  action: 'Approve pending vault payment', cluster: config.cluster,
  feePayer: owner.publicKey.toBase58(),
  recipients: [{ address: data.merchantTokens.toBase58(), amount: data.amount.toString(), token: config.mint }],
});
const signature = await provider.sendAndConfirm(transaction, [owner]);
console.log(JSON.stringify({ receipt: receipt.toBase58(), signature, explorer: `https://explorer.solana.com/tx/${signature}?cluster=${config.cluster}` }, null, 2));
