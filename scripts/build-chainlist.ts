import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';
import {
  CHAIN_DIR,
  OUTFILE_CHAIN,
  SUPPORTED_CHAIN_MAP,
  TOKENLIST_BASE_URL,
} from '../config';

const chainsData: Record<string, string> = JSON.parse(
  fs.readFileSync(path.join(CHAIN_DIR, 'chains.json'), 'utf8'),
);

const chainlist: Record<number, string> = {};

for (const [chainName, logoFilename] of Object.entries(chainsData)) {
  if (chainName === '$schema' || !logoFilename) continue;

  const chain =
    SUPPORTED_CHAIN_MAP[chainName as keyof typeof SUPPORTED_CHAIN_MAP];
  if (!chain) {
    console.warn(`Unknown chain in chains.json: ${chainName}`);
    continue;
  }

  chainlist[chain.id] = url.resolve(
    TOKENLIST_BASE_URL,
    path.join(CHAIN_DIR, logoFilename),
  );
}

fs.writeFileSync(OUTFILE_CHAIN, JSON.stringify(chainlist, null, 2));
