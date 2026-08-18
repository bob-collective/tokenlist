import type { Address } from 'viem';
import type { SUPPORTED_CHAIN_MAP, SUPPORTED_CHAINS } from './config';
import type { TokenId } from './token-ids';

export type KebabCase<T extends string> = T extends `${infer S} ${infer E}`
  ? `${Lowercase<S>}-${KebabCase<E>}`
  : // biome-ignore lint/suspicious/noExplicitAny: any
    T extends `${infer S}${infer E extends `${string}${any}`}`
    ? E extends Uppercase<E>
      ? `${Lowercase<S>}-${Lowercase<E>}`
      : `${Lowercase<S>}${KebabCase<E>}`
    : Lowercase<T>;

export type ValueOf<T> = T[keyof T];
export type Entries<T> = [keyof T, ValueOf<T>][];

export type SupportedChain = keyof typeof SUPPORTED_CHAIN_MAP;
export type SupportedChainId = (typeof SUPPORTED_CHAINS)[number]['id'];

type Overrides = Partial<Pick<TokenData, 'name' | 'symbol' | 'decimals'>>;

export type TokenData = {
  name: string;
  symbol: string;
  decimals: number;
  native?: boolean;
  coingeckoId: string;
  underlying?: TokenId;
  description?: string;
  website?: string;
  twitter?: string;
  tokens: Record<
    SupportedChain,
    {
      address: Address;
      name?: string;
      symbol?: string;
      decimals?: number;
      native?: boolean;
      bridge?: Record<SupportedChain, Address>;
      overrides?: Overrides;
    }
  >;
};

// Compressed tokenlist tuple shapes (see scripts/build-compressedlist.ts)
// [name, symbol, coingeckoId, logo, underlying?]
export type CompressedSharedEntry = [string, string, string, string, TokenId?];
// [chainId, address, decimals, native, nameOverride?, symbolOverride?]
export type CompressedChainEntry =
  | [number, string, number, boolean]
  | [number, string, number, boolean, string]
  | [number, string, number, boolean, string | null, string];
// first element = shared data, rest = per-chain entries
export type CompressedEntry = [
  CompressedSharedEntry,
  ...CompressedChainEntry[],
];

export type Token = {
  name: string;
  address: Address;
  symbol: string;
  decimals: number;
  chainId: number;
  logoURI: string;
  extensions: {
    tokenId: TokenId;
    coingeckoId: string;
    underlying?: TokenId;
    native: boolean;
    bridge?: Record<SupportedChainId, Address>;
  };
};
