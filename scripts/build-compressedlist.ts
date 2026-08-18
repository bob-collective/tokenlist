import fs from 'node:fs';
import path from 'node:path';
import { glob } from 'glob';
import {
  NON_EVM_CHAIN_ID_BY_NAME,
  OUTFILE_COMPRESSED,
  SUPPORTED_CHAIN_MAP,
  TOKEN_DIR,
} from '../config';
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

function isSolanaChain(chain: string): boolean {
  return chain === 'solana';
}

function resolveChainId(chain: string): number {
  const evmChain =
    SUPPORTED_CHAIN_MAP[chain as keyof typeof SUPPORTED_CHAIN_MAP];
  if (evmChain) return evmChain.id;

  const nonEvmId = NON_EVM_CHAIN_ID_BY_NAME[chain];
  if (nonEvmId !== undefined) return nonEvmId;

  throw new Error(`Unknown chain in token data: ${chain}`);
}

function formatAddress(address: string, chain: string): string {
  return isSolanaChain(chain) ? address : checksumAddress(address);
}

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
      resolveChainId(chain),
      formatAddress(token.address, chain),
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

  const shared: CompressedEntry[0] = data.underlying
    ? [data.name, data.symbol, data.coingeckoId, logo, data.underlying]
    : [data.name, data.symbol, data.coingeckoId, logo];

  compressedlist[folder as TokenId] = [shared, ...chains];
}

fs.writeFileSync(OUTFILE_COMPRESSED, JSON.stringify(compressedlist, null, 2));
