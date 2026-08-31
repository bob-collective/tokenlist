import fs from 'node:fs';
import path from 'node:path';
import { glob } from 'glob';
import { bob } from 'viem/chains';
import {
  NON_EVM_CHAIN_ID_BY_NAME,
  OUTFILE_BOB,
  OUTFILE_OVERRIDES,
  OUTFILE_TOKENLIST,
  SUPPORTED_CHAIN_MAP,
  TOKEN_DIR,
  TOKENLIST_BASE_URL,
  TOKENLIST_SCHEMA_URL,
} from '../config';
import { version } from '../package.json';
import type { TokenId } from '../token-ids';
import type { Entries, Token, TokenData } from '../types';
import { checksumAddress } from '../utils';

const [major, minor, patch] = version.split('.');

// Solana addresses are base58 with no checksum concept — pass them through
// untouched; everything else is normalised to an EVM/Tron checksummed address.
function isSolanaChain(chain: string): boolean {
  return chain === 'solana';
}

// Resolve a data.json chain-name key to its numeric chain ID, covering both
// viem-described EVM chains and non-EVM chains registered in config.
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

function buildTokenlist(tokens: Token[][]) {
  return tokens.reduce(
    (list, tokens) => {
      list.tokens.push(...tokens);

      return list;
    },
    {
      $schema: TOKENLIST_SCHEMA_URL,
      name: 'BOB Tokens',
      timestamp: new Date().toISOString(),
      version: {
        major: parseInt(major, 10),
        minor: parseInt(minor, 10),
        patch: parseInt(patch, 10),
      },
      tokens: [] as Token[],
    },
  );
}

function mapToTokenlist(data: [TokenId, TokenData, string][]) {
  return data.map(([tokenId, tokenData, logoURI]) => {
    return (
      Object.entries(tokenData.tokens) as Entries<typeof tokenData.tokens>
    ).map(([chain, token]) => {
      const bridge = (
        Object.entries(token.bridge || {}) as Entries<
          NonNullable<typeof token.bridge>
        >
      ).reduce(
        (acc, [chain, bridgeAddress]) => {
          acc[resolveChainId(chain)] = bridgeAddress;

          return acc;
        },
        {} as Record<number, string>,
      );

      return {
        chainId: resolveChainId(chain),
        address: formatAddress(token.address, chain),
        name: token.name ?? tokenData.name,
        symbol: token.symbol ?? tokenData.symbol,
        decimals: token.decimals ?? tokenData.decimals,
        logoURI,
        extensions: {
          tokenId,
          coingeckoId: tokenData.coingeckoId,
          native: token.native ?? tokenData.native ?? false,
          bridge,
        },
      } as Token;
    });
  });
}

function mapToOverridesTokenlist(data: [TokenId, TokenData, string][]) {
  return data.map(([tokenId, tokenData, logoURI]) => {
    return (
      Object.entries(tokenData.tokens) as Entries<typeof tokenData.tokens>
    ).map(([chain, token]) => {
      const bridge = (
        Object.entries(token.bridge || {}) as Entries<
          NonNullable<typeof token.bridge>
        >
      ).reduce(
        (acc, [chain, bridgeAddress]) => {
          acc[resolveChainId(chain)] = bridgeAddress;

          return acc;
        },
        {} as Record<number, string>,
      );

      return {
        chainId: resolveChainId(chain),
        address: formatAddress(token.address, chain),
        name: token.overrides?.name ?? token.name ?? tokenData.name,
        symbol: token.overrides?.symbol ?? token.symbol ?? tokenData.symbol,
        decimals:
          token.overrides?.decimals ?? token.decimals ?? tokenData.decimals,
        logoURI,
        extensions: {
          tokenId,
          coingeckoId: tokenData.coingeckoId,
          native: token.native ?? tokenData.native ?? false,
          bridge,
        },
      } as Token;
    });
  });
}

const tokenlistData = fs
  .readdirSync(TOKEN_DIR)
  .sort((a, b) => {
    return a.toLowerCase().localeCompare(b.toLowerCase());
  })
  .map<[TokenId, TokenData, string]>((folder) => {
    const data: TokenData = JSON.parse(
      fs.readFileSync(path.join(TOKEN_DIR, folder, 'data.json'), 'utf8'),
    );
    const logofiles = glob.sync(
      path.join(TOKEN_DIR, folder, 'logo.{webp,svg}'),
    );
    const logoext = logofiles[0].endsWith('webp') ? 'webp' : 'svg';

    return [
      folder as TokenId,
      data,
      new URL(
        path.posix.join(TOKEN_DIR, folder, `logo.${logoext}`),
        TOKENLIST_BASE_URL,
      ).toString(),
    ];
  });

// Build tokenlist
const tokenlist = buildTokenlist(mapToTokenlist(tokenlistData));

fs.writeFileSync(OUTFILE_TOKENLIST, JSON.stringify(tokenlist, null, 2));

// Build BOB tokenlist
const bobTokenlist = structuredClone(tokenlist);

bobTokenlist.tokens = tokenlist.tokens.filter(
  (token) => token.chainId === bob.id,
);

fs.writeFileSync(OUTFILE_BOB, JSON.stringify(bobTokenlist, null, 2));

// Build tokenlist with overrides
const uiTokenlist = buildTokenlist(mapToOverridesTokenlist(tokenlistData));

fs.writeFileSync(OUTFILE_OVERRIDES, JSON.stringify(uiTokenlist, null, 2));
