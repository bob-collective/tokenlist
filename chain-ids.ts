// Non-EVM chains have no viem definition, so their chain IDs live here. Solana
// uses its genesis-derived numeric chain ID; the Bitcoin networks use their
// genesis block timestamps (mainnet 2009-01-03, signet 2020-09-01) since
// Bitcoin has no native chain-ID concept.
//
// This module imports nothing, so `@gobob/tokenlist/chain-ids` gives consumers
// these IDs without the viem chain definitions built in `config.ts`.
export const SOLANA_CHAIN_ID = 1584368940;
export const BITCOIN_CHAIN_ID = 1231006505;
export const SIGNET_CHAIN_ID = 1598918400;
