# @gobob/tokenlist

Authoritative token registry for the BOB ecosystem. The package publishes token list JSON files, TypeScript token identifiers, native-token identifiers, bridge metadata, and optional UI overrides from the source data in [`data/`](./data).

## Contents

- [Installation](#installation)
- [Usage](#usage)
- [Logo URLs](#logo-urls)
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
import tokenList from "@gobob/tokenlist/tokenlist.json";

// CommonJS
const tokenList = require("@gobob/tokenlist/tokenlist.json");
```

### Exports

| File                              | Description                                                                                                                       |
| --------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `tokenlist.json`                  | Complete token list across all chains                                                                                             |
| `tokenlist-bob.json`              | Tokens on BOB chain only                                                                                                          |
| `tokenlist-overrides.json`        | Token list with UI overrides applied                                                                                              |
| `tokenlist-mirror.json`           | `tokenlist.json` with R2 mirror logo URIs                                                                                         |
| `tokenlist-bob-mirror.json`       | `tokenlist-bob.json` with R2 mirror logo URIs                                                                                     |
| `tokenlist-overrides-mirror.json` | `tokenlist-overrides.json` with R2 mirror logo URIs                                                                               |
| `chainlist.json`                  | Chain ID to logo URI map (GitHub-hosted logos)                                                                                    |
| `chainlist-mirror.json`           | Chain ID to logo URI map (R2 mirror logos)                                                                                        |
| `compressedlist.json`             | Size-optimized token list; each `TokenId` maps to `[sharedTuple, ...chainTuples]` (see [Compact Token List](#compact-token-list)) |
| `token-ids.ts`                    | Generated token identifier types                                                                                                  |

### TypeScript Types

The package exports TypeScript types for type-safe development:

```typescript
import type { NativeTokenId, TokenId } from "@gobob/tokenlist/token-ids";
import type { Token, TokenData, SupportedChain } from "@gobob/tokenlist/types";
```

| Type             | Description                                                      |
| ---------------- | ---------------------------------------------------------------- |
| `TokenId`        | Union of all token identifiers (e.g., `'WBTC' \| 'USDT' \| ...`) |
| `NativeTokenId`  | Union of token identifiers marked with `"native": true`          |
| `Token`          | Single token object from the tokenlist                           |
| `TokenData`      | Token metadata structure used in `data.json` files               |
| `SupportedChain` | Union of supported chain names                                   |

---

## Logo URLs

Token logos are served from two hosts. Both helpers are exported from the package root, take the same arguments, and return the same path — only the host differs:

| Helper              | Host                                                                | Use when                                       |
| ------------------- | ------------------------------------------------------------------- | ---------------------------------------------- |
| `getLogoURI`        | `https://raw.githubusercontent.com/bob-collective/tokenlist/refs/heads/main/` | Default; logos read straight from GitHub       |
| `getMirrorLogoURI`  | `https://static.gobob.xyz/tokenlist/`                               | Prefer the BOB R2 mirror (CDN-backed, cached)  |

```typescript
import { getLogoURI, getMirrorLogoURI } from "@gobob/tokenlist";

getLogoURI("USDC", "svg");
// "https://raw.githubusercontent.com/bob-collective/tokenlist/refs/heads/main/data/tokens/USDC/logo.svg"

getMirrorLogoURI("USDC", "svg");
// "https://static.gobob.xyz/tokenlist/data/tokens/USDC/logo.svg"
```

The same split exists in the generated files: every `-mirror` list (`tokenlist-mirror.json`, `tokenlist-bob-mirror.json`, `tokenlist-overrides-mirror.json`, `chainlist-mirror.json`) is byte-identical to its counterpart apart from the logo host. Use `getMirrorLogoURI` when reconstructing logo URLs yourself (e.g. from `compressedlist.json`); use the `-mirror` lists when consuming prebuilt JSON.

Mirror assets are published by the `Publish tokenlist assets` workflow after every merge to `main` — see [Asset publishing](#asset-publishing).

---

## Compact Token List

> **Use only when bundle size is critical.** `tokenlist.json` is the canonical, complete list — prefer it unless you must minimize shipped bytes.

`compressedlist.json` is a size-optimized form of the token list. Each `TokenId` maps to an array whose **first element** is the shared per-token metadata tuple `[name, symbol, coingeckoId, logo]`, followed by one per-chain tuple `[chainId, address, decimals, native, nameOverride?, symbolOverride?]` for each chain the token is on. Storing shared metadata once (instead of repeating it per chain) and dropping logo URLs — reconstructed at runtime via `getLogoURI` (or `getMirrorLogoURI`) — compresses to a fraction of `tokenlist.json`, at the cost of reassembly work at runtime.

The trailing `nameOverride`/`symbolOverride` slots carry the per-chain UI overrides from `tokenlist-overrides.json` and are appended only when present:

- Both overrides set → `[..., native, nameOverride, symbolOverride]`
- Only `symbolOverride` set → `[..., native, null, symbolOverride]` (name slot held by `null`)
- Only `nameOverride` set → `[..., native, nameOverride]`
- Neither → tuple ends at `native` (length 4)

Shape:

```jsonc
{
  "USDC": [
    // [0] shared — [name, symbol, coingeckoId, logo]
    // logo = "svg" | "webp"
    ["USD Coin", "USDC", "usd-coin", "svg"],
    // [1..] per-chain — [chainId, address, decimals, native, nameOverride?, symbolOverride?]
    [1, "0xA0b8...", 6, false],
    // with both overrides
    [60808, "0xe75D...", 6, false, "Bridged USDC", "USDC.e"],
    // symbol-only override — name slot is null
    [130, "0x9151...", 6, false, null, "USDT0"],
  ],
}
```

Reconstruct the flat token list by splitting the shared head from the chain tail and rebuilding each logo URL with `getLogoURI`:

```typescript
import compressed from "@gobob/tokenlist/compressedlist.json";
import { getLogoURI } from "@gobob/tokenlist";
import type { TokenId } from "@gobob/tokenlist/token-ids";

const tokens = Object.entries(compressed).flatMap(([id, entry]) => {
  const [[name, symbol, coingeckoId, logoext], ...chains] = entry;

  return chains.map(
    ([chainId, address, decimals, native, nameOverride, symbolOverride]) => ({
      chainId,
      address,
      name: nameOverride ?? name,
      symbol: symbolOverride ?? symbol,
      decimals,
      logoURI: getLogoURI(id as TokenId, logoext),
      extensions: { tokenId: id, coingeckoId, native },
    }),
  );
});
```

To read logos from the BOB R2 mirror instead of GitHub, swap in `getMirrorLogoURI` — same signature, same return shape, only the host changes:

```typescript
import { getMirrorLogoURI } from "@gobob/tokenlist";

logoURI: getMirrorLogoURI(id as TokenId, logoext),
```

**Limitations:** this compact form omits `extensions.bridge`. Per-chain `name`/`symbol` overrides are carried via the trailing tuple slots, but `decimals` overrides are not — the `decimals` slot always holds the effective value. If you need bridge data, use `tokenlist.json` or `tokenlist-overrides.json` instead.

---

## Data Model

Each token lives in its own directory under [`data/`](./data). The directory name is the canonical token identifier and is used to generate `TokenId`.

```text
data/
└── tokens/
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
    },
    "solana": {
      "address": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
    }
  }
}
```

Top-level fields:

| Field         | Required | Description                                                                                                                                                                                                         |
| ------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `name`        | Yes      | Default token name                                                                                                                                                                                                  |
| `symbol`      | Yes      | Default token symbol                                                                                                                                                                                                |
| `decimals`    | Yes      | Default token decimals                                                                                                                                                                                              |
| `tokens`      | Yes      | Per-chain token records keyed by supported chain name                                                                                                                                                               |
| `coingeckoId` | Yes      | CoinGecko API ID used for price feeds (e.g., `usd-coin`). Emitted as `extensions.coingeckoId` in the generated token lists. For receipt or wrapped tokens without their own listing, use the underlying asset's ID. |
| `native`      | No       | Marks native chain assets such as ETH, BNB, POL, or TLOS                                                                                                                                                            |
| `description` | No       | Project or token description                                                                                                                                                                                        |
| `website`     | No       | Project website URL                                                                                                                                                                                                 |
| `twitter`     | No       | Project Twitter/X handle                                                                                                                                                                                            |

Per-chain `tokens` entries:

| Field       | Required | Description                                                                                           |
| ----------- | -------- | ----------------------------------------------------------------------------------------------------- |
| `address`   | Yes      | Token address on that chain. Use `0x0000000000000000000000000000000000000000` for most native assets. |
| `name`      | No       | Chain-specific token name used in the base token lists                                                |
| `symbol`    | No       | Chain-specific symbol used in the base token lists                                                    |
| `decimals`  | No       | Chain-specific decimals when they differ from the top-level value                                     |
| `bridge`    | No       | Map of related chain name to bridge token address                                                     |
| `overrides` | No       | UI-facing `name`, `symbol`, or `decimals` used only in `tokenlist-overrides.json`                     |

Chain keys must match the supported chain names in [`config.ts`](./config.ts). Update [`token.schema.json`](./token.schema.json) when adding a new supported chain so editors keep autocompletion and validation.

Solana uses base58 token mint addresses and is emitted with chain ID `1399811149`. Solana addresses are passed through unchanged because they do not have EVM checksum semantics.

---

## Adding New Tokens

### 1. Create Token Directory

```text
data/
└── tokens/
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
    },
    "solana": {
      "address": "..."
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
- `tokenlist-mirror.json`, `tokenlist-bob-mirror.json`, `tokenlist-overrides-mirror.json` — same lists with R2 mirror logo URIs
- `chainlist.json` — supported chain logo map (GitHub-hosted logos)
- `chainlist-mirror.json` — supported chain logo map (R2 mirror logos)
- `compressedlist.json` — compact shared + per-chain tuple output

### 5. Verify

```bash
pnpm verify
```

---

## Scripts

| Command                     | Description                                                                 |
| --------------------------- | --------------------------------------------------------------------------- |
| `pnpm build`                | Generate types, build token lists, then run verification                    |
| `pnpm build:tokenlist`      | Generate tokenlist JSON files (GitHub-hosted and `-mirror` variants)        |
| `pnpm build:compressedlist` | Generate `compressedlist.json` (compact shared + per-chain tuples)          |
| `pnpm build:chainlist`      | Generate `chainlist.json` and `chainlist-mirror.json`                       |
| `pnpm build:types`          | Generate `TokenId` and `NativeTokenId` TypeScript unions                    |
| `pnpm check`                | Run Biome formatting, import organization, and lint checks                  |
| `pnpm check:write`          | Apply safe Biome formatting/import/lint fixes                               |
| `pnpm format`               | Check formatting with Biome                                                 |
| `pnpm format:write`         | Apply Biome formatting                                                      |
| `pnpm lint`                 | Run Biome lint rules                                                        |
| `pnpm publish:assets`       | Publish token and chain logos to Cloudflare R2 and verify the public copies |
| `pnpm verify`               | Validate tokenlist schema and on-chain token data                           |

---

## Asset publishing

The `Publish tokenlist assets` workflow uploads token and chain logos to
`https://static.gobob.xyz/tokenlist/` after every merge to `main`, verifies the public copies, and only then notifies
the UI repository to update its tokenlist pin. Generated token and chain lists retain their GitHub-hosted logo URLs;
consumers can call `getMirrorLogoURI` instead of `getLogoURI` (or rewrite the
`https://raw.githubusercontent.com/bob-collective/tokenlist/refs/heads/main` prefix themselves) when
loading logos from the R2 mirror, or consume the `-mirror` lists (`tokenlist-mirror.json`,
`tokenlist-bob-mirror.json`, `tokenlist-overrides-mirror.json`, `chainlist-mirror.json`), which are generated with
mirror URLs already applied. Each `-mirror` file is byte-identical to its counterpart apart from the logo host.

Configure the workflow's `production` environment with an R2 Object Read & Write token restricted to the bucket
behind `static.gobob.xyz`:

- `R2_ACCOUNT_ID`
- `R2_ACCESS_KEY_ID`
- `R2_SECRET_ACCESS_KEY`
- `R2_BUCKET_NAME`
- `UI_DISPATCH_TOKEN`, with permission to dispatch events to `bob-collective/ui`

R2 objects are stored below the `tokenlist/` prefix and retain the existing mutable-path behavior with a five-minute
browser cache lifetime. Publishing is idempotent and does not delete assets that are no longer referenced.

---

## Notes

**Native tokens:** Native chain assets should set `"native": true` and use the zero address where the asset is native. They are exported through `NativeTokenId` and marked as `extensions.native` in the generated token lists.

**CoinGecko IDs:** Each token's `coingeckoId` is emitted as `extensions.coingeckoId` in the generated token lists, making the tokenlist the single source of truth for price feed IDs.

**Generated files:** Do not edit `token-ids.ts`, `tokenlist.json`, `tokenlist-bob.json`, `tokenlist-overrides.json`, any `-mirror.json` list, `chainlist.json`, or `chainlist-mirror.json` by hand. Update `data/tokens/[TOKEN]/data.json` and run `pnpm build`.

---

## Contributing

Open a PR against `bob-collective/tokenlist` with your changes.

## License

MIT
