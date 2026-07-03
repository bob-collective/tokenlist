import fs from 'node:fs';
import path from 'node:path';
import { OUTFILE_SHARED, TOKEN_DIR } from '../config';
import type { TokenId } from '../token-ids';
import type { TokenData } from '../types';

// tokenId -> [name, symbol, coingeckoId]
type SharedEntry = [string, string, string];

const sharedlist: Record<TokenId, SharedEntry> = {} as Record<
  TokenId,
  SharedEntry
>;

const folders = fs.readdirSync(TOKEN_DIR).sort((a, b) => {
  return a.toLowerCase().localeCompare(b.toLowerCase());
});

for (const folder of folders) {
  const data: TokenData = JSON.parse(
    fs.readFileSync(path.join(TOKEN_DIR, folder, 'data.json'), 'utf8'),
  );

  sharedlist[folder as TokenId] = [data.name, data.symbol, data.coingeckoId];
}

fs.writeFileSync(OUTFILE_SHARED, JSON.stringify(sharedlist, null, 2));
