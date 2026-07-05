# @gobob/tokenlist

Authoritative token registry for the BOB ecosystem. The package publishes token list JSON files, TypeScript token identifiers, native-token identifiers, bridge metadata, and optional UI overrides from the source data in [`data/`](./data).

## Contents

- [Installation](#installation)
- [Usage](#usage)
- [Compact Token List](#compact-token-list)
- [Data Model](#data-model)
- [Adding New Tokens](#adding-new-tokens)
- [Scripts](#scripts)

---

## Installation

```json
{
  "@gobob/tokenlist": "github:bob-collective/tokenlist#<commit_hash>"
}
```

```bash
pnpm install
```

---

## Usage

```typescript
// ESM
import tokenList from '@gobob/tokenlist/tokenlist.json';

// CommonJS
const tokenList = require('@gobob/tokenlist/tokenlist.json');
```

### Exports

| File | Description |
|------|-------------|
| `tokenlist.json` | Complete token list across all chains |
| `tokenlist-bob.json` | Tokens on BOB chain only |
| `tokenlist-overrides.json` | Token list with UI overrides applied |
| `compressedlist.json` | Size-optimized token list; each `TokenId` maps to `[sharedTuple, ...chainTuples]` (see [Compact Token List](#compact-token-list)) |
| `token-ids.ts` | Generated token identifier types |

### TypeScript Types

The package exports TypeScript types for type-safe development:

```typescript
import type { NativeTokenId, TokenId } from '@gobob/tokenlist/token-ids';
import type { Token, TokenData, SupportedChain } from '@gobob/tokenlist/types';
```

| Type | Description |
|------|-------------|
| `TokenId` | Union of all token identifiers (e.g., `'WBTC' \| 'USDT' \| ...`) |
| `NativeTokenId` | Union of token identifiers marked with `"native": true` |
| `Token` | Single token object from the tokenlist |
| `TokenData` | Token metadata structure used in `data.json` files |
| `SupportedChain` | Union of supported chain names |

---

## Compact Token List

> **Use only when bundle size is critical.** `tokenlist.json` is the canonical, complete list — prefer it unless you must minimize shipped bytes.

`compressedlist.json` is a size-optimized form of the token list. Each `TokenId` maps to an array whose **first element** is the shared per-token metadata tuple `[name, symbol, coingeckoId]`, followed by one per-chain tuple `[chainId, address, decimals, logo, native]` for each chain the token is on. Storing shared metadata once (instead of repeating it per chain) and dropping logo URLs — reconstructed at runtime via `getLogoURI` — compresses to a fraction of `tokenlist.json`, at the cost of reassembly work at runtime.

Shape:

```jsonc
{
  "USDC": [
    // [0] shared — [name, symbol, coingeckoId]
    ["USD Coin", "USDC", "usd-coin"],
    // [1..] per-chain — [chainId, address, decimals, logo, native]  (logo = "svg" | "webp")
    [1, "0xA0b8...", 6, "svg", false],
    [56, "0x8AC7...", 18, "svg", false]
  ]
}
```

Reconstruct the flat token list by splitting the shared head from the chain tail and rebuilding each logo URL with `getLogoURI`:

```typescript
import compressed from '@gobob/tokenlist/compressedlist.json';
import { getLogoURI } from '@gobob/tokenlist';
import type { TokenId } from '@gobob/tokenlist/token-ids';

const tokens = Object.entries(compressed).flatMap(([id, entry]) => {
  const [[name, symbol, coingeckoId], ...chains] = entry;

  return chains.map(([chainId, address, decimals, logo, native]) => ({
    chainId,
    address,
    name,
    symbol,
    decimals,
    logoURI: getLogoURI(id as TokenId, logo),
    extensions: { tokenId: id, coingeckoId, native },
  }));
});
```

**Limitations:** this compact form omits data the full lists carry — `extensions.bridge` and per-chain `name`/`symbol`/`decimals` overrides. If you need either, use `tokenlist.json` or `tokenlist-overrides.json` instead.

---

## Data Model

Each token lives in its own directory under [`data/`](./data). The directory name is the canonical token identifier and is used to generate `TokenId`.

```text
data/
└── USDC/
    ├── data.json
    └── logo.svg
```

### `data.json`

The source file contains shared token metadata plus per-chain entries under `tokens`:

```json
{
  "$schema": "../../../token.schema.json",
  "name": "USD Coin",
  "symbol": "USDC",
  "decimals": 6,
  "coingeckoId": "usd-coin",
  "description": "USDC is a digital dollar issued by Circle.",
  "website": "https://www.usdc.com/",
  "twitter": "@circle",
  "tokens": {
    "ethereum": {
      "address": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
      "bridge": {
        "bob": "0x450D55a4B4136805B0e5A6BB59377c71FC4FaCBb"
      }
    },
    "bob": {
      "name": "Bridged USDC (BOB)",
      "symbol": "USDC.e",
      "address": "0xe75D0fB2C24A55cA1e3F96781a2bCC7bdba058F0",
      "bridge": {
        "ethereum": "0xe497788F8Fcc30B773C9A181a0FFE2e60645cE90"
      },
      "overrides": {
        "name": "Bridged USDC",
        "symbol": "USDC.e"
      }
    }
  }
}
```

Top-level fields:

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Default token name |
| `symbol` | Yes | Default token symbol |
| `decimals` | Yes | Default token decimals |
| `tokens` | Yes | Per-chain token records keyed by supported chain name |
| `coingeckoId` | Yes | CoinGecko API ID used for price feeds (e.g., `usd-coin`). Emitted as `extensions.coingeckoId` in the generated token lists. For receipt or wrapped tokens without their own listing, use the underlying asset's ID. |
| `native` | No | Marks native chain assets such as ETH, BNB, POL, or TLOS |
| `description` | No | Project or token description |
| `website` | No | Project website URL |
| `twitter` | No | Project Twitter/X handle |

Per-chain `tokens` entries:

| Field | Required | Description |
|-------|----------|-------------|
| `address` | Yes | Token address on that chain. Use `0x0000000000000000000000000000000000000000` for most native assets. |
| `name` | No | Chain-specific token name used in the base token lists |
| `symbol` | No | Chain-specific symbol used in the base token lists |
| `decimals` | No | Chain-specific decimals when they differ from the top-level value |
| `bridge` | No | Map of related chain name to bridge token address |
| `overrides` | No | UI-facing `name`, `symbol`, or `decimals` used only in `tokenlist-overrides.json` |

Chain keys must match the supported chain names in [`config.ts`](./config.ts). Update [`token.schema.json`](./token.schema.json) when adding a new supported chain so editors keep autocompletion and validation.

---

## Adding New Tokens

### 1. Create Token Directory

```text
data/
└── MYTOKEN/
    ├── data.json
    └── logo.{svg|webp}
```

### 2. Add Token Metadata

Create `data.json`:

```json
{
  "$schema": "../../../token.schema.json",
  "name": "My Token",
  "symbol": "MYTOKEN",
  "decimals": 18,
  "coingeckoId": "my-token",
  "description": "Short project or token description.",
  "website": "https://example.com",
  "twitter": "@example",
  "tokens": {
    "ethereum": {
      "address": "0x...",
      "bridge": {
        "bob": "0x..."
      }
    },
    "bob": {
      "address": "0x...",
      "overrides": {
        "name": "My Token",
        "symbol": "MYTOKEN"
      }
    }
  }
}
```

For native assets, add `"native": true` at the top level and use the zero address for the native chain entries.

Set `coingeckoId` to the token's API ID from the [CoinGecko coins list](https://api.coingecko.com/api/v3/coins/list) so price feeds work downstream. If the token has no listing of its own (e.g., a lending receipt token), use the underlying asset's ID.

### 3. Add Logo

Add `logo.svg` or `logo.webp` (min 200x200px for raster).

### 4. Build

**Required:** After adding a token, regenerate the JSON files:

```bash
pnpm build
```

This updates:
- `token-ids.ts` — generated `TokenId` and `NativeTokenId` unions
- `tokenlist.json` — all tokens using base names, symbols, and decimals
- `tokenlist-bob.json` — BOB chain tokens
- `tokenlist-overrides.json` — tokens with overrides applied

### 5. Verify

```bash
pnpm verify
```

---

## Scripts

| Command | Description |
|---------|-------------|
| `pnpm build` | Generate types, build token lists, then run verification |
| `pnpm build:tokenlist` | Generate the 3 tokenlist JSON files |
| `pnpm build:compressedlist` | Generate `compressedlist.json` (compact `tokens` + `shared` maps) |
| `pnpm build:types` | Generate `TokenId` and `NativeTokenId` TypeScript unions |
| `pnpm check` | Run Biome formatting, import organization, and lint checks |
| `pnpm check:write` | Apply safe Biome formatting/import/lint fixes |
| `pnpm format` | Check formatting with Biome |
| `pnpm format:write` | Apply Biome formatting |
| `pnpm lint` | Run Biome lint rules |
| `pnpm verify` | Validate tokenlist schema and on-chain token data |

---

## Notes

**Native tokens:** Native chain assets should set `"native": true` and use the zero address where the asset is native. They are exported through `NativeTokenId` and marked as `extensions.native` in the generated token lists.

**CoinGecko IDs:** Each token's `coingeckoId` is emitted as `extensions.coingeckoId` in the generated token lists, making the tokenlist the single source of truth for price feed IDs.

**Generated files:** Do not edit `token-ids.ts`, `tokenlist.json`, `tokenlist-bob.json`, or `tokenlist-overrides.json` by hand. Update `data/[TOKEN]/data.json` and run `pnpm build`.

---

## Contributing

Open a PR against `bob-collective/tokenlist` with your changes.

## License

MIT
