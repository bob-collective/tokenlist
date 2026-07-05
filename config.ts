import { defineChain } from 'viem';
import {
  arbitrum,
  avalanche,
  base,
  berachain,
  bob,
  bobSepolia,
  bsc,
  hyperEvm,
  mainnet,
  optimism,
  plasma,
  polygon,
  sei,
  sepolia,
  soneium,
  sonic,
  telos,
  tron,
  unichain,
} from 'viem/chains';
import { mapByName } from './utils';

export const TOKENLIST_SCHEMA_URL =
  'https://raw.githubusercontent.com/Uniswap/token-lists/refs/heads/main/src/tokenlist.schema.json';
export const TOKENLIST_BASE_URL =
  'https://raw.githubusercontent.com/bob-collective/tokenlist/refs/heads/main/';

export const TOKEN_DIR = './data/tokens';
export const OUTFILE_TOKENLIST = 'tokenlist.json';
export const OUTFILE_BOB = 'tokenlist-bob.json';
export const OUTFILE_OVERRIDES = 'tokenlist-overrides.json';
export const OUTFILE_TYPES = 'token-ids.ts';

export const CHAIN_DIR = './data/chains';
export const OUTFILE_CHAIN = 'chainlist.json';

export const OUTFILE_COMPRESSED = 'compressedlist.json';

const supportedMainnetChains = [
  defineChain({
    ...mainnet,
    rpcUrls: {
      default: {
        http: ['https://ethereum-rpc.publicnode.com'],
      },
    },
  }),
  bob,
  bsc,
  base,
  defineChain({
    ...optimism,
    rpcUrls: {
      default: {
        http: ['https://optimism-rpc.publicnode.com'],
      },
    },
  }),
  arbitrum,
  polygon,
  avalanche,
  unichain,
  sei,
  soneium,
  berachain,
  sonic,
  telos,
  defineChain({
    ...hyperEvm,
    contracts: {
      multicall3: {
        address: '0xcA11bde05977b3631167028862bE2a173976CA11',
      },
    },
  }),
  plasma,
  defineChain({
    ...tron,
    contracts: {
      multicall3: {
        address: '0x32a4f47a74a6810bd0bf861cabab99656a75de9e',
      },
    },
  }),
];
const supportedTestnetChains = [sepolia, bobSepolia];
export const SUPPORTED_CHAINS = [
  ...supportedMainnetChains,
  ...supportedTestnetChains,
];

export const SUPPORTED_MAINNET_CHAINS = mapByName(supportedMainnetChains);
export const SUPPORTED_TESTNET_CHAINS = mapByName(supportedTestnetChains);
export const SUPPORTED_CHAIN_MAP = mapByName(SUPPORTED_CHAINS);
