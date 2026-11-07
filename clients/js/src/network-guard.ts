const DEVNET_GENESIS_HASH =
  'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG';

export type SolanaNetwork = 'devnet' | 'localnet';

type JsonRpcResponse = {
  result?: string;
  error?: { code?: number; message?: string };
};

export async function assertSolanaNetwork(
  rpcUrl: string,
  network: SolanaNetwork,
  options: { localnetGenesisHash?: string } = {},
): Promise<string> {
  const expectedGenesisHash =
    network === 'devnet'
      ? DEVNET_GENESIS_HASH
      : options.localnetGenesisHash ?? process.env.SOLANA_LOCALNET_GENESIS_HASH;

  if (!expectedGenesisHash) {
    throw new Error(
      'Localnet requires SOLANA_LOCALNET_GENESIS_HASH so the RPC network can be verified.',
    );
  }

  let response: Response;
  try {
    response = await fetch(rpcUrl, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: 'agent-vault-network-guard',
        method: 'getGenesisHash',
      }),
    });
  } catch (error) {
    throw new Error(
      `Could not reach Solana RPC at ${rpcUrl}.`,
      { cause: error },
    );
  }

  if (!response.ok) {
    throw new Error(
      `Solana RPC at ${rpcUrl} returned HTTP ${response.status} while checking the network.`,
    );
  }

  const payload = (await response.json()) as JsonRpcResponse;
  if (payload.error || !payload.result) {
    throw new Error(
      `Solana RPC at ${rpcUrl} did not return a genesis hash: ${
        payload.error?.message ?? 'unknown RPC response'
      }.`,
    );
  }

  if (payload.result !== expectedGenesisHash) {
    throw new Error(
      `Refusing RPC ${rpcUrl}: expected ${network} genesis hash ${expectedGenesisHash}, got ${payload.result}.`,
    );
  }

  return payload.result;
}
