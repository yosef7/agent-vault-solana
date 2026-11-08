import { readFile, writeFile, mkdir, chmod } from 'node:fs/promises';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as anchor from '@coral-xyz/anchor';
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  createTransferCheckedInstruction,
  createMint,
  getAccount,
  getAssociatedTokenAddressSync,
  getMint,
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
import { confirmTransaction } from './transaction-approval.js';

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
  merchants: { research: string; unknown: string; external: string };
  merchantTokens: { research: string; unknown: string; external: string };
  parameters: {
    deposit: string;
    dailyLimit: string;
    approvalThreshold: string;
    queryPrice: string;
    reportPrice: string;
  };
};

function requireJson(path: string): Uint8Array | undefined {
  try {
    const parsed: unknown = JSON.parse(readFileSync(path, 'utf8'));
    if (!Array.isArray(parsed) || parsed.length !== 64 ||
        parsed.some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255)) {
      throw new Error(`Invalid keypair file ${path}: expected 64 integer bytes.`);
    }
    return Uint8Array.from(parsed as number[]);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return undefined;
    throw error;
  }
}

async function loadKeypair(path: string): Promise<Keypair> {
  const secretKey = requireJson(path);
  if (!secretKey) {
    const keypair = Keypair.generate();
    await saveKeypair(path, keypair);
    return keypair;
  }
  const keypair = Keypair.fromSecretKey(secretKey);
  await chmod(path, 0o600);
  return keypair;
}

async function saveKeypair(path: string, keypair: Keypair): Promise<void> {
  await writeFile(path, JSON.stringify(Array.from(keypair.secretKey)) + '\n', {
    mode: 0o600,
    flag: 'wx',
  }).catch(async (error: NodeJS.ErrnoException) => {
    if (error.code !== 'EEXIST') throw error;
    await chmod(path, 0o600);
    const existing = requireJson(path);
    if (!existing || !Buffer.from(existing).equals(Buffer.from(keypair.secretKey))) {
      throw new Error(`Refusing to overwrite mismatched keypair at ${path}.`);
    }
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
  const maria = await loadKeypair(resolve(keysDir, 'maria-keypair.json'));
  const agent = await loadKeypair(resolve(keysDir, 'agent-keypair.json'));
  const research = await loadKeypair(
    resolve(keysDir, 'merchant-research-keypair.json'),
  );
  const unknown = await loadKeypair(
    resolve(keysDir, 'merchant-unknown-keypair.json'),
  );
  const external = await loadKeypair(
    resolve(keysDir, 'merchant-external-keypair.json'),
  );
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
  const mintAccount = await getMint(connection, mint);
  if (mintAccount.decimals !== decimals || mintAccount.freezeAuthority !== null) {
    throw new Error('Configured mint does not match the expected six-decimal, non-freezable demo mint.');
  }
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
  const externalTokens = await getOrCreateAssociatedTokenAccount(
    connection,
    maria,
    mint,
    external.publicKey,
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
      vault: { fetchNullable(address: PublicKey): Promise<{
        authority: PublicKey; agent: PublicKey; merchants: PublicKey[];
        merchantCount: number; mint: PublicKey; dailyLimit: anchor.BN;
        approvalThreshold: anchor.BN;
      } | null> };
    }
  ).vault.fetchNullable(vault);
  const merchants = [
    research.publicKey,
    unknown.publicKey,
    PublicKey.default,
    PublicKey.default,
    PublicKey.default,
  ];
  if (account) {
    const matches = account.authority.equals(maria.publicKey) &&
      account.agent.equals(agent.publicKey) && account.mint.equals(mint) &&
      account.merchantCount === 2 &&
      account.merchants.every((key, index) => key.equals(merchants[index]!)) &&
      account.dailyLimit.eq(new anchor.BN(10_000_000)) &&
      account.approvalThreshold.eq(new anchor.BN(1_000_000));
    if (!matches) throw new Error('Existing vault configuration does not match this setup; refusing to fund it.');
  } else {
    const transaction = await program.methods
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
      .transaction();
    await provider.simulate(transaction, [maria]);
    await confirmTransaction(transaction, {
      action: 'Initialize Agent Vault', cluster: network,
      feePayer: maria.publicKey.toBase58(),
      recipients: [{ address: vault.toBase58() }, { address: vaultTokens.address.toBase58(), token: mint.toBase58() }],
    });
    await provider.sendAndConfirm(transaction, [maria]);
  }

  const vaultTokenAccount = await getAccount(connection, vaultTokens.address);
  if (!vaultTokenAccount.owner.equals(vault) || !vaultTokenAccount.mint.equals(mint)) {
    throw new Error('Vault token account does not match the validated vault and mint.');
  }

  const vaultBalance = await getAccount(connection, vaultTokens.address);
  if (vaultBalance.amount < 50_000_000n) {
    const transaction = new Transaction().add(
        createTransferCheckedInstruction(
          mariaTokens.address,
          mint,
          vaultTokens.address,
          maria.publicKey,
          50_000_000,
          decimals,
        ),
      );
    await connection.simulateTransaction(transaction);
    await confirmTransaction(transaction, {
      action: 'Deposit demo tokens into Agent Vault', cluster: network,
      feePayer: maria.publicKey.toBase58(),
      recipients: [{ address: vaultTokens.address.toBase58(), amount: '50000000', token: mint.toBase58() }],
    });
    await sendAndConfirmTransaction(connection, transaction, [maria]);
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
      external: external.publicKey.toBase58(),
    },
    merchantTokens: {
      research: researchTokens.address.toBase58(),
      unknown: unknownTokens.address.toBase58(),
      external: externalTokens.address.toBase58(),
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
