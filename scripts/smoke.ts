import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import * as anchor from '@coral-xyz/anchor';
import { PublicKey, SystemProgram } from '@solana/web3.js';
import { ASSOCIATED_TOKEN_PROGRAM_ID, TOKEN_PROGRAM_ID } from '@solana/spl-token';

const root = resolve(new URL('.', import.meta.url).pathname, '..');
const config = JSON.parse(
  await readFile(resolve(root, 'config/devnet.json'), 'utf8'),
) as {
  rpcUrl: string;
  programId: string;
  vault: string;
  vaultTokens: string;
  mint: string;
  merchantTokens: { research: string };
};
const agent = anchor.web3.Keypair.fromSecretKey(
  Uint8Array.from(
    JSON.parse(
      await readFile(resolve(root, 'keys/agent-keypair.json'), 'utf8'),
    ),
  ),
);
const provider = new anchor.AnchorProvider(
  new anchor.web3.Connection(config.rpcUrl, 'confirmed'),
  new anchor.Wallet(agent),
  { commitment: 'confirmed' },
);
const idl = JSON.parse(
  await readFile(resolve(root, 'target/idl/agent_vault.json'), 'utf8'),
) as anchor.Idl;
const program = new anchor.Program(idl, provider);
const requestHash = createHash('sha256')
  .update(`demo-${Date.now()}`)
  .digest();
const [receipt] = PublicKey.findProgramAddressSync(
  [Buffer.from('receipt'), new PublicKey(config.vault).toBuffer(), requestHash],
  new PublicKey(config.programId),
);
const signature = await program.methods
  .requestPayment(Array.from(requestHash), new anchor.BN(10_000))
  .accounts({
    vault: config.vault,
    receipt,
    vaultTokens: config.vaultTokens,
    merchantTokens: config.merchantTokens.research,
    mint: config.mint,
    agent: agent.publicKey,
    tokenProgram: TOKEN_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
  })
  .rpc();
const result = await (
  program.account as unknown as {
    receipt: {
      fetch(address: PublicKey): Promise<{
        amount: anchor.BN;
        status: unknown;
        requestHash: number[];
      }>;
    };
  }
).receipt.fetch(receipt);
console.log(
  JSON.stringify(
    {
      signature,
      receipt: receipt.toBase58(),
      amount: result.amount.toString(),
      status: result.status,
      requestHash: Buffer.from(result.requestHash).toString('hex'),
    },
    null,
    2,
  ),
);
