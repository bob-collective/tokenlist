import fs from 'node:fs';
import path from 'node:path';
import {
  CHAIN_DIR,
  NON_EVM_CHAIN_ID_BY_NAME,
  NON_EVM_CHAIN_IDS,
  NON_EVM_TESTNET_CHAIN_NAMES,
  OUTFILE_CHAIN,
  OUTFILE_CHAIN_MIRROR,
  OUTFILE_SUPPORTED_CHAINS,
  SUPPORTED_CHAIN_MAP,
  SUPPORTED_TESTNET_CHAINS,
  TOKENLIST_BASE_URL,
  TOKENLIST_MIRROR_URL,
} from '../config';

const chainsData: Record<string, string> = JSON.parse(
  fs.readFileSync(path.join(CHAIN_DIR, 'chains.json'), 'utf8'),
);

// Both maps are keyed by numeric chain ID so downstream consumers can map a
// token's chainId (incl. non-EVM chains like Solana) directly to its logo.
// They differ only in host: GitHub raw for the default list, the R2 mirror for
// consumers that cannot depend on raw.githubusercontent.com.
const chainlist: Record<number, string> = {};
const mirrorChainlist: Record<number, string> = {};

for (const [chainName, logoFilename] of Object.entries(chainsData)) {
  if (chainName === '$schema' || !logoFilename) continue;

  const chainId =
    SUPPORTED_CHAIN_MAP[chainName as keyof typeof SUPPORTED_CHAIN_MAP]?.id ??
    NON_EVM_CHAIN_ID_BY_NAME[chainName];

  if (chainId === undefined) {
    console.warn(`Unknown chain in chains.json: ${chainName}`);
    continue;
  }

  const logoPath = path.posix.join(CHAIN_DIR, logoFilename);

  chainlist[chainId] = new URL(logoPath, TOKENLIST_BASE_URL).toString();
  mirrorChainlist[chainId] = new URL(logoPath, TOKENLIST_MIRROR_URL).toString();
}

fs.writeFileSync(OUTFILE_CHAIN, JSON.stringify(chainlist, null, 2));
fs.writeFileSync(
  OUTFILE_CHAIN_MIRROR,
  JSON.stringify(mirrorChainlist, null, 2),
);

// Every supported chain keyed by its data.json name, in SUPPORTED_CHAIN_IDS
// order, for consumers that cannot import config.ts.
const supportedChains = Object.fromEntries([
  ...Object.entries(SUPPORTED_CHAIN_MAP).map(([chainName, chain]) => [
    chainName,
    {
      chainId: chain.id,
      name: chain.name,
      evm: !NON_EVM_CHAIN_IDS.has(chain.id),
      testnet: chainName in SUPPORTED_TESTNET_CHAINS,
    },
  ]),
  ...Object.entries(NON_EVM_CHAIN_ID_BY_NAME).map(([chainName, chainId]) => [
    chainName,
    {
      chainId,
      name: chainName.charAt(0).toUpperCase() + chainName.slice(1),
      evm: false,
      testnet: NON_EVM_TESTNET_CHAIN_NAMES.has(chainName),
    },
  ]),
]);

fs.writeFileSync(
  OUTFILE_SUPPORTED_CHAINS,
  JSON.stringify(supportedChains, null, 2),
);
