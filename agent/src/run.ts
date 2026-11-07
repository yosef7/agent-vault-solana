import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import * as anchor from '@coral-xyz/anchor';
import { PublicKey, SystemProgram } from '@solana/web3.js';
import { TOKEN_PROGRAM_ID } from '@solana/spl-token';
import { assertSolanaNetwork } from '../../clients/js/src/network-guard.js';

type Scenario = '1' | '2' | '3';
type Config = {
  cluster: 'devnet' | 'localnet';
  rpcUrl: string;
  programId: string;
  vault: string;
  vaultTokens: string;
  mint: string;
  agent: string;
  merchants: { research: string; unknown: string; external: string };
  merchantTokens: { research: string; unknown: string; external: string };
  parameters: { queryPrice: string; reportPrice: string };
};
type Decision = {
  task: string;
  api: 'weather' | 'premium-report' | 'external';
  amount: number;
  reason: string;
  nonce: string;
};

const root = resolve(new URL('.', import.meta.url).pathname, '../..');
const config = JSON.parse(
  await readFile(resolve(root, 'config/devnet.json'), 'utf8'),
) as Config;
const scenario = parseScenario(process.argv.slice(2));

function parseScenario(args: string[]): Scenario {
  const value = args[args.indexOf('--scenario') + 1];
  if (value === '1' || value === '2' || value === '3') return value;
  throw new Error('Usage: npm run agent -- --scenario 1|2|3');
}

const catalog = {
  weather: {
    name: 'Panama Weather API',
    merchant: 'research' as const,
    price: Number(config.parameters.queryPrice),
    description: 'Current weather for Panama City',
  },
  'premium-report': {
    name: 'Premium Research API',
    merchant: 'research' as const,
    price: Number(config.parameters.reportPrice),
    description: 'A premium market report',
  },
  external: {
    name: 'External Unlisted API',
    merchant: 'external' as const,
    price: Number(config.parameters.queryPrice),
    description: 'An API outside the vault allowlist',
  },
} as const;

const scenarioTasks: Record<Scenario, { task: string; api: keyof typeof catalog }> = {
  '1': { task: 'Get the weather in Panama City and translate it to English.', api: 'weather' },
  '2': { task: 'Buy the premium research report.', api: 'premium-report' },
  '3': { task: 'Use an external provider that is not in the approved list.', api: 'external' },
};

async function decide(): Promise<Decision> {
  const selected = scenarioTasks[scenario];
  const nonce = `${scenario}-${Date.now()}`;
  if (!process.env.ANTHROPIC_API_KEY) {
    return {
      task: selected.task,
      api: selected.api,
      amount: catalog[selected.api].price,
      reason: `Scenario ${scenario}: selected ${catalog[selected.api].name} because it matches the requested task.`,
      nonce,
    };
  }

  const response = await fetch('https://api.anthropic.com/v1/messages', {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      'x-api-key': process.env.ANTHROPIC_API_KEY,
      'anthropic-version': '2023-06-01',
    },
    body: JSON.stringify({
      model: process.env.ANTHROPIC_MODEL ?? 'claude-sonnet-5-5',
      max_tokens: 300,
      system: 'Return only valid JSON with keys api, amount, reason. Choose the requested scenario exactly.',
      messages: [{
        role: 'user',
        content: JSON.stringify({
          task: selected.task,
          requiredApi: selected.api,
          catalog,
          instruction: 'Use the requiredApi and its listed price. Explain the choice briefly.',
        }),
      }],
    }),
  });
  if (!response.ok) {
    throw new Error(`Anthropic API returned HTTP ${response.status}.`);
  }
  const payload = (await response.json()) as { content?: Array<{ text?: string }> };
  const text = payload.content?.[0]?.text;
  if (!text) throw new Error('Anthropic API returned no text content.');
  const parsed = JSON.parse(text) as { api: keyof typeof catalog; amount: number; reason: string };
  if (parsed.api !== selected.api || parsed.amount !== catalog[selected.api].price || !parsed.reason) {
    throw new Error('Model decision did not match the fixed demo scenario or catalog price.');
  }
  return { task: selected.task, api: parsed.api, amount: parsed.amount, reason: parsed.reason, nonce };
}

const decision = await decide();
const canonical = JSON.stringify({
  task: decision.task,
  api: decision.api,
  amount: decision.amount,
  reason: decision.reason,
  nonce: decision.nonce,
});
const requestHash = createHash('sha256').update(canonical).digest();
const [receipt] = PublicKey.findProgramAddressSync(
  [Buffer.from('receipt'), new PublicKey(config.vault).toBuffer(), requestHash],
  new PublicKey(config.programId),
);
await mkdir(resolve(root, 'agent/receipts'), { recursive: true });
await writeFile(
  resolve(root, 'agent/receipts', `${requestHash.toString('hex')}.json`),
  `${JSON.stringify({ ...decision, canonical, requestHash: requestHash.toString('hex') }, null, 2)}\n`,
);

await assertSolanaNetwork(config.rpcUrl, config.cluster);
const agent = anchor.web3.Keypair.fromSecretKey(
  Uint8Array.from(JSON.parse(await readFile(resolve(root, 'keys/agent-keypair.json'), 'utf8'))),
);
const provider = new anchor.AnchorProvider(
  new anchor.web3.Connection(config.rpcUrl, 'confirmed'),
  new anchor.Wallet(agent),
  { commitment: 'confirmed' },
);
const idl = JSON.parse(await readFile(resolve(root, 'target/idl/agent_vault.json'), 'utf8')) as anchor.Idl;
const program = new anchor.Program(idl, provider);
const transaction = await program.methods
  .requestPayment(Array.from(requestHash), new anchor.BN(decision.amount))
  .accounts({
    vault: config.vault,
    receipt,
    vaultTokens: config.vaultTokens,
    merchantTokens: config.merchantTokens[catalog[decision.api].merchant],
    mint: config.mint,
    agent: agent.publicKey,
    tokenProgram: TOKEN_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
  })
  .transaction();
await provider.simulate(transaction, [agent]);
const signature = await provider.sendAndConfirm(transaction, [agent]);
const result = await (program.account as unknown as {
  receipt: { fetch(address: PublicKey): Promise<{ status: unknown; reason: unknown; requestHash: number[] }> };
}).receipt.fetch(receipt);
const explorer = `https://explorer.solana.com/tx/${signature}?cluster=${config.cluster}`;
console.log(JSON.stringify({
  scenario, api: decision.api, amount: decision.amount, receipt: receipt.toBase58(),
  status: result.status, reason: result.reason, signature, explorer,
  requestHash: Buffer.from(result.requestHash).toString('hex'),
}, null, 2));
