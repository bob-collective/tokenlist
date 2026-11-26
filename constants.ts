// Plain values with no imports, so a consumer can import
// `@gobob/tokenlist/constants` without the viem chains built in `config.ts`.

// Non-EVM chains have no viem definition, so their chain IDs live here. Solana
// uses its genesis-derived numeric chain ID; the Bitcoin networks use their
// genesis block timestamps (mainnet 2009-01-03, signet 2020-09-01) since
// Bitcoin has no native chain-ID concept.
export const SOLANA_CHAIN_ID = 1584368940;
export const BITCOIN_CHAIN_ID = 1231006505;
export const SIGNET_CHAIN_ID = 1598918400;

export const TOKEN_DIR = './data/tokens';
export const TOKENLIST_BASE_URL =
  'https://raw.githubusercontent.com/bob-collective/tokenlist/refs/heads/main/';
export const TOKENLIST_MIRROR_URL = 'https://static.gobob.xyz/tokenlist/';
