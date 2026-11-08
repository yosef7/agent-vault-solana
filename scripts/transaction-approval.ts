import { createInterface } from 'node:readline/promises';
import type { Transaction } from '@solana/web3.js';

export async function confirmTransaction(
  transaction: Transaction,
  context: {
    action: string;
    cluster: string;
    feePayer: string;
    recipients: Array<{ address: string; amount?: string; token?: string }>;
  },
): Promise<void> {
  if (!process.stdin.isTTY || !process.stdout.isTTY) {
    throw new Error('Refusing to send: transaction approval requires an interactive terminal.');
  }

  const summary = {
    action: context.action,
    cluster: context.cluster,
    feePayer: context.feePayer,
    recipients: context.recipients,
    fee: 'Network fee determined by Solana at confirmation',
    instructions: transaction.instructions.length,
  };
  console.log('Simulated transaction. Review before signing:');
  console.log(JSON.stringify(summary, null, 2));
  const readline = createInterface({ input: process.stdin, output: process.stdout });
  try {
    const answer = await readline.question('Type SEND to sign and broadcast this transaction: ');
    if (answer !== 'SEND') throw new Error('Transaction cancelled.');
  } finally {
    readline.close();
  }
}
