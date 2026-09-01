import fs from 'node:fs';
import path from 'node:path';
import { glob } from 'glob';
import { bob } from 'viem/chains';
import {
  NON_EVM_CHAIN_ID_BY_NAME,
  OUTFILE_BOB,
  OUTFILE_BOB_MIRROR,
  OUTFILE_OVERRIDES,
  OUTFILE_OVERRIDES_MIRROR,
  OUTFILE_TOKENLIST,
  OUTFILE_TOKENLIST_MIRROR,
  SUPPORTED_CHAIN_MAP,
  TOKEN_DIR,
  TOKENLIST_SCHEMA_URL,
} from '../config';
import { version } from '../package.json';
import type { TokenId } from '../token-ids';
import type { Entries, Token, TokenData } from '../types';
import { checksumAddress, getLogoURI, getMirrorLogoURI } from '../utils';

const [major, minor, patch] = version.split('.');

// Shared across every generated list so a mirror file differs from its
// GitHub-hosted counterpart only by logo host.
const timestamp = new Date().toISOString();

type LogoExtension = 'svg' | 'webp';
type TokenEntry = [TokenId, TokenData, LogoExtension];
type LogoURIResolver = (tokenId: TokenId, logoext: LogoExtension) => string;

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
      timestamp,
      version: {
        major: parseInt(major, 10),
        minor: parseInt(minor, 10),
        patch: parseInt(patch, 10),
      },
      tokens: [] as Token[],
    },
  );
}

function mapToTokenlist(data: TokenEntry[], getLogo: LogoURIResolver) {
  return data.map(([tokenId, tokenData, logoext]) => {
    const logoURI = getLogo(tokenId, logoext);

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

function mapToOverridesTokenlist(data: TokenEntry[], getLogo: LogoURIResolver) {
  return data.map(([tokenId, tokenData, logoext]) => {
    const logoURI = getLogo(tokenId, logoext);

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
  .map<TokenEntry>((folder) => {
    const data: TokenData = JSON.parse(
      fs.readFileSync(path.join(TOKEN_DIR, folder, 'data.json'), 'utf8'),
    );
    const logofiles = glob.sync(
      path.join(TOKEN_DIR, folder, 'logo.{webp,svg}'),
    );
    const logoext = logofiles[0].endsWith('webp') ? 'webp' : 'svg';

    return [folder as TokenId, data, logoext];
  });

// One set of lists per logo host: GitHub raw for the default files, the R2
// mirror for the `-mirror` files. Content is otherwise identical.
function writeTokenlists(
  getLogo: LogoURIResolver,
  outfiles: { tokenlist: string; bob: string; overrides: string },
): void {
  const tokenlist = buildTokenlist(mapToTokenlist(tokenlistData, getLogo));

  fs.writeFileSync(outfiles.tokenlist, JSON.stringify(tokenlist, null, 2));

  const bobTokenlist = structuredClone(tokenlist);

  bobTokenlist.tokens = tokenlist.tokens.filter(
    (token) => token.chainId === bob.id,
  );

  fs.writeFileSync(outfiles.bob, JSON.stringify(bobTokenlist, null, 2));

  const uiTokenlist = buildTokenlist(
    mapToOverridesTokenlist(tokenlistData, getLogo),
  );

  fs.writeFileSync(outfiles.overrides, JSON.stringify(uiTokenlist, null, 2));
}

writeTokenlists(getLogoURI, {
  tokenlist: OUTFILE_TOKENLIST,
  bob: OUTFILE_BOB,
  overrides: OUTFILE_OVERRIDES,
});

writeTokenlists(getMirrorLogoURI, {
  tokenlist: OUTFILE_TOKENLIST_MIRROR,
  bob: OUTFILE_BOB_MIRROR,
  overrides: OUTFILE_OVERRIDES_MIRROR,
});
