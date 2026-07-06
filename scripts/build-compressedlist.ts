import fs from 'node:fs';
import path from 'node:path';
import { glob } from 'glob';
import { OUTFILE_COMPRESSED, SUPPORTED_CHAIN_MAP, TOKEN_DIR } from '../config';
import type { TokenId } from '../token-ids';
import type {
  CompressedChainEntry,
  CompressedEntry,
  Entries,
  TokenData,
} from '../types';
import { checksumAddress } from '../utils';

const compressedlist: Record<TokenId, CompressedEntry> = {} as Record<
  TokenId,
  CompressedEntry
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

  const chains = (
    Object.entries(data.tokens) as Entries<typeof data.tokens>
  ).map<CompressedChainEntry>(([chain, token]) => {
    const base: [number, string, number, boolean] = [
      SUPPORTED_CHAIN_MAP[chain].id,
      checksumAddress(token.address),
      token.overrides?.decimals ?? token.decimals ?? data.decimals,
      token.native ?? data.native ?? false,
    ];

    const nameOverride = token.overrides?.name ?? token.name;
    const symbolOverride = token.overrides?.symbol ?? token.symbol;
    const hasNameOverride =
      nameOverride !== undefined && nameOverride !== data.name;
    const hasSymbolOverride =
      symbolOverride !== undefined && symbolOverride !== data.symbol;

    if (hasSymbolOverride) {
      return [...base, hasNameOverride ? nameOverride : null, symbolOverride];
    }
    if (hasNameOverride) {
      return [...base, nameOverride];
    }

    return base;
  });

  compressedlist[folder as TokenId] = [
    [data.name, data.symbol, data.coingeckoId, logo],
    ...chains,
  ];
}

fs.writeFileSync(OUTFILE_COMPRESSED, JSON.stringify(compressedlist, null, 2));
