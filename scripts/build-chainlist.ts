import fs from 'node:fs';
import path from 'node:path';
import {
  CHAIN_DIR,
  NON_EVM_CHAIN_ID_BY_NAME,
  OUTFILE_CHAIN,
  SUPPORTED_CHAIN_MAP,
  TOKENLIST_BASE_URL,
} from '../config';

const chainsData: Record<string, string> = JSON.parse(
  fs.readFileSync(path.join(CHAIN_DIR, 'chains.json'), 'utf8'),
);

// Keyed by numeric chain ID so downstream consumers can map a token's chainId
// (incl. non-EVM chains like Solana) directly to its logo.
const chainlist: Record<number, string> = {};

for (const [chainName, logoFilename] of Object.entries(chainsData)) {
  if (chainName === '$schema' || !logoFilename) continue;

  const chainId =
    SUPPORTED_CHAIN_MAP[chainName as keyof typeof SUPPORTED_CHAIN_MAP]?.id ??
    NON_EVM_CHAIN_ID_BY_NAME[chainName];

  if (chainId === undefined) {
    console.warn(`Unknown chain in chains.json: ${chainName}`);
    continue;
  }

  chainlist[chainId] = new URL(
    path.posix.join(CHAIN_DIR, logoFilename),
    TOKENLIST_BASE_URL,
  ).toString();
}

fs.writeFileSync(OUTFILE_CHAIN, JSON.stringify(chainlist, null, 2));
