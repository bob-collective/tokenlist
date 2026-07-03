import fs from 'node:fs';
import path from 'node:path';
import { glob } from 'glob';
import { OUTFILE_COMPRESSED, SUPPORTED_CHAIN_MAP, TOKEN_DIR } from '../config';
import type { TokenId } from '../token-ids';
import type { Entries, TokenData } from '../types';
import { checksumAddress } from '../utils';

// [chainId, address, decimals, logo, native]
type CompressedEntry = [number, string, number, string, boolean];

const compressedlist: Record<TokenId, CompressedEntry[]> = {} as Record<
  TokenId,
  CompressedEntry[]
>;

const folders = fs.readdirSync(TOKEN_DIR).sort((a, b) => {
  return a.toLowerCase().localeCompare(b.toLowerCase());
});

for (const folder of folders) {
  const data: TokenData = JSON.parse(
    fs.readFileSync(path.join(TOKEN_DIR, folder, 'data.json'), 'utf8'),
  );

  const logofiles = glob.sync(path.join(TOKEN_DIR, folder, 'logo.{webp,svg}'));
  const logo = logofiles[0].endsWith('webp') ? 'webp' : 'svg';

  compressedlist[folder as TokenId] = (
    Object.entries(data.tokens) as Entries<typeof data.tokens>
  ).map(([chain, token]) => {
    return [
      SUPPORTED_CHAIN_MAP[chain].id,
      checksumAddress(token.address),
      token.decimals ?? data.decimals,
      logo,
      token.native ?? data.native ?? false,
    ];
  });
}

fs.writeFileSync(OUTFILE_COMPRESSED, JSON.stringify(compressedlist, null, 2));
