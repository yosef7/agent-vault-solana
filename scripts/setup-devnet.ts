import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as anchor from '@coral-xyz/anchor';
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  createAssociatedTokenAccount,
  createTransferCheckedInstruction,
  createMint,
  getAccount,
  getAssociatedTokenAddressSync,
  getOrCreateAssociatedTokenAccount,
  mintTo,
} from '@solana/spl-token';
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
} from '@solana/web3.js';
import { assertSolanaNetwork } from '../clients/js/src/network-guard.js';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const rpcUrl = process.env.SOLANA_RPC_URL ?? 'https://api.devnet.solana.com';
const network = process.env.SOLANA_CLUSTER ?? 'devnet';
const programId = new PublicKey(
  process.env.AGENT_VAULT_PROGRAM_ID ??
    '41pzYtwB4MJV2c7uyhEqSUYc3awtekzsPiAGLiPr4Fvi',
);
const decimals = 6;
const configPath = resolve(root, 'config/devnet.json');
const keysDir = resolve(root, 'keys');

type DemoConfig = {
  cluster: 'devnet';
  rpcUrl: string;
  programId: string;
  mint: string;
  vault: string;
  vaultTokens: string;
  maria: string;
  agent: string;
  merchants: { research: string; unknown: string };
  merchantTokens: { research: string; unknown: string };
  parameters: {
    deposit: string;
    dailyLimit: string;
    approvalThreshold: string;
    queryPrice: string;
    reportPrice: string;
  };
};

function loadOrCreateKeypair(path: string): Keypair {
  return Keypair.fromSecretKey(
    requireJson(path) ??
      (() => {
        const keypair = Keypair.generate();
        return keypair.secretKey;
      })(),
  );
}

function requireJson(path: string): Uint8Array | undefined {
  try {
    return Uint8Array.from(JSON.parse(readFileSync(path, 'utf8')));
  } catch {
    return undefined;
  }
}

async function saveKeypair(path: string, keypair: Keypair): Promise<void> {
  await writeFile(path, JSON.stringify(Array.from(keypair.secretKey)) + '\n', {
    mode: 0o600,
  });
}

async function ensureSol(connection: Connection, payer: Keypair, recipient: PublicKey) {
  const balance = await connection.getBalance(recipient);
  if (balance >= 0.2 * anchor.web3.LAMPORTS_PER_SOL) return;
  // The devnet RPC faucet is closed (HTTP 410), so fund from the payer wallet.
  const tx = new anchor.web3.Transaction().add(
    anchor.web3.SystemProgram.transfer({
      fromPubkey: payer.publicKey,
      toPubkey: recipient,
      lamports: 0.2 * anchor.web3.LAMPORTS_PER_SOL,
    }),
  );
  await anchor.web3.sendAndConfirmTransaction(connection, tx, [payer], {
    commitment: 'confirmed',
  });
}

async function main(): Promise<void> {
  if (network !== 'devnet' && network !== 'localnet') {
    throw new Error(`Unsupported SOLANA_CLUSTER "${network}". Use devnet or localnet.`);
  }
  await assertSolanaNetwork(rpcUrl, network);
  await mkdir(keysDir, { recursive: true });
  const maria = loadOrCreateKeypair(resolve(keysDir, 'maria-keypair.json'));
  const agent = loadOrCreateKeypair(resolve(keysDir, 'agent-keypair.json'));
  const research = loadOrCreateKeypair(
    resolve(keysDir, 'merchant-research-keypair.json'),
  );
  const unknown = loadOrCreateKeypair(
    resolve(keysDir, 'merchant-unknown-keypair.json'),
  );
  await Promise.all([
    saveKeypair(resolve(keysDir, 'maria-keypair.json'), maria),
    saveKeypair(resolve(keysDir, 'agent-keypair.json'), agent),
    saveKeypair(resolve(keysDir, 'merchant-research-keypair.json'), research),
    saveKeypair(resolve(keysDir, 'merchant-unknown-keypair.json'), unknown),
  ]);

  const devWallet = anchor.Wallet.local();
  const connection = new Connection(rpcUrl, 'confirmed');
  await ensureSol(connection, devWallet.payer, maria.publicKey);
  await ensureSol(connection, devWallet.payer, agent.publicKey);

  const existing = await readConfig();
  const mint = existing?.mint ? new PublicKey(existing.mint) : await createMint(
    connection,
    maria,
    maria.publicKey,
    null,
    decimals,
  );
  const researchTokens = await getOrCreateAssociatedTokenAccount(
    connection,
    maria,
    mint,
    research.publicKey,
  );
  const unknownTokens = await getOrCreateAssociatedTokenAccount(
    connection,
    maria,
    mint,
    unknown.publicKey,
  );

  const vault = PublicKey.findProgramAddressSync(
    [Buffer.from('vault'), maria.publicKey.toBuffer()],
    programId,
  )[0];
  // Only derive the address: `initialize` creates this account with `init`,
  // so pre-creating it makes the Associated Token program fail.
  const vaultTokens = {
    address: getAssociatedTokenAddressSync(mint, vault, true),
  };
  const mariaTokens = await getOrCreateAssociatedTokenAccount(
    connection,
    maria,
    mint,
    maria.publicKey,
  );
  const agentTokens = await getOrCreateAssociatedTokenAccount(
    connection,
    agent,
    mint,
    agent.publicKey,
  );
  if ((await getAccount(connection, mariaTokens.address)).amount === 0n) {
    await mintTo(connection, maria, mint, mariaTokens.address, maria, 50_000_000);
  }

  const provider = new anchor.AnchorProvider(
    connection,
    new anchor.Wallet(maria),
    { commitment: 'confirmed' },
  );
  const idl = JSON.parse(
    await readFile(resolve(root, 'target/idl/agent_vault.json'), 'utf8'),
  ) as anchor.Idl;
  const program = new anchor.Program(idl, provider);
  const account = await (
    program.account as unknown as {
      vault: { fetchNullable(address: PublicKey): Promise<unknown> };
    }
  ).vault.fetchNullable(vault);
  if (!account) {
    const merchants = [
      research.publicKey,
      unknown.publicKey,
      PublicKey.default,
      PublicKey.default,
      PublicKey.default,
    ];
    await program.methods
      .initialize(
        agent.publicKey,
        merchants,
        2,
        new anchor.BN(10_000_000),
        new anchor.BN(1_000_000),
      )
      .accounts({
        authority: maria.publicKey,
        vault,
        vaultTokens: vaultTokens.address,
        mint,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([maria])
      .rpc();
  }

  const vaultBalance = await getAccount(connection, vaultTokens.address);
  if (vaultBalance.amount < 50_000_000n) {
    await sendAndConfirmTransaction(
      connection,
      new Transaction().add(
        createTransferCheckedInstruction(
          mariaTokens.address,
          mint,
          vaultTokens.address,
          maria.publicKey,
          50_000_000,
          decimals,
        ),
      ),
      [maria],
    );
  }

  const config: DemoConfig = {
    cluster: 'devnet',
    rpcUrl,
    programId: programId.toBase58(),
    mint: mint.toBase58(),
    vault: vault.toBase58(),
    vaultTokens: vaultTokens.address.toBase58(),
    maria: maria.publicKey.toBase58(),
    agent: agent.publicKey.toBase58(),
    merchants: {
      research: research.publicKey.toBase58(),
      unknown: unknown.publicKey.toBase58(),
    },
    merchantTokens: {
      research: researchTokens.address.toBase58(),
      unknown: unknownTokens.address.toBase58(),
    },
    parameters: {
      deposit: '50000000',
      dailyLimit: '10000000',
      approvalThreshold: '1000000',
      queryPrice: '10000',
      reportPrice: '5000000',
    },
  };
  await mkdir(resolve(root, 'config'), { recursive: true });
  await writeFile(configPath, JSON.stringify(config, null, 2) + '\n');
  console.log(`Devnet ready. Vault: ${vault.toBase58()}`);
}

async function readConfig(): Promise<DemoConfig | undefined> {
  try {
    return JSON.parse(await readFile(configPath, 'utf8')) as DemoConfig;
  } catch {
    return undefined;
  }
}

main().catch((error: unknown) => {
  console.error(error);
  process.exitCode = 1;
});
