# @gobob/tokenlist

Authoritative token registry for the BOB ecosystem. The package publishes token list JSON files, TypeScript token identifiers, native-token identifiers, bridge metadata, and optional UI overrides from the source data in [`data/`](./data).

The same data ships as a Rust crate, `gobob-tokenlist`, with typed token IDs, chains, token data, and the same utilities — see [Rust](#rust).

## Contents

- [Installation](#installation)
- [Usage](#usage)
- [Logo URLs](#logo-urls)
- [Compact Token List](#compact-token-list)
- [Rust](#rust)
- [Data Model](#data-model)
- [Adding New Tokens](#adding-new-tokens)
- [Scripts](#scripts)

---

## Installation

JavaScript / TypeScript:

```json
{
  "@gobob/tokenlist": "github:bob-collective/tokenlist#<commit_hash>"
}
```

```bash
pnpm install
```

Rust: see [Rust](#rust).

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
| `supported-chains.json`           | Supported chains keyed by `data.json` name: `chainId`, display `name`, `evm`, `testnet`                                           |
| `compressedlist.json`             | Size-optimized token list; each `TokenId` maps to `[sharedTuple, ...chainTuples]` (see [Compact Token List](#compact-token-list)) |
| `token-ids.ts`                    | Generated token identifier types                                                                                                  |
| `token-ids.json`                  | `TokenId`, `NativeTokenId`, `NonEvmTokenId`, `NonEvmNativeTokenId` as JSON arrays                                                 |

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

## Rust

The repository root is also a Rust crate, `gobob-tokenlist`, built from the same [`data/`](./data). It is distributed as a Git dependency (not on crates.io) and needs Rust 1.88+.

```toml
[dependencies]
gobob-tokenlist = { git = "https://github.com/bob-collective/tokenlist", rev = "<commit_hash>" }
```

```bash
cargo add gobob-tokenlist --git https://github.com/bob-collective/tokenlist --rev <commit_hash>
```

Token data is compiled into the crate: no I/O or JSON parsing at runtime. Bump `rev` to pick up new tokens.

```rust
use gobob_tokenlist::{tokens, tokens_with_overrides, Chain, NativeTokenId, TokenId};

// Token IDs are enums; parse and display them with the original ID string.
let id: TokenId = "BTC.b".parse()?;
assert_eq!(id, TokenId::BTC_b);
assert_eq!(id.as_str(), "BTC.b");

// data.json contents
let usdc = TokenId::USDC.data();
assert_eq!((usdc.symbol, usdc.decimals, usdc.coingecko_id), ("USDC", 6, "usd-coin"));

// tokenlist-overrides.json entries (use `tokens()` for tokenlist.json)
let usdc_on_bob = tokens_with_overrides()
    .find(|token| token.token_id == TokenId::USDC && token.chain == Chain::Bob)
    .unwrap();
assert_eq!(usdc_on_bob.symbol, "USDC.e");
assert_eq!(usdc_on_bob.chain_id(), 60808);
let logo = usdc_on_bob.logo_uri(); // or .mirror_logo_uri()

// Look a token up by chain and address (any letter case; Tron also as 0x hex)
let usdc_on_base = Chain::Base.token("0x833589fcd6edb6e08f4c7c32d4f71b54bda02913").unwrap();
assert_eq!((usdc_on_base.token_id, usdc_on_base.decimals), (TokenId::USDC, 6));

// BOB-only list, like tokenlist-bob.json
let bob_tokens: Vec<_> = tokens().filter(|token| token.chain == Chain::Bob).collect();

// Subsets convert to and from TokenId
let eth: NativeTokenId = TokenId::ETH.try_into()?;
assert_eq!(TokenId::from(eth), TokenId::ETH);
assert!(NativeTokenId::try_from(TokenId::USDC).is_err());

// Chains
assert_eq!(Chain::from_id(60808), Some(Chain::Bob));
assert_eq!("op-mainnet".parse::<Chain>()?, Chain::OpMainnet);
assert!(!Chain::Solana.is_evm());
```

### npm ↔ Rust

| npm package                                                                          | Rust crate                                                                                                     |
| ------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------- |
| `TokenId`, `NativeTokenId`, `NonEvmTokenId`, `NonEvmNativeTokenId`                   | Enums of the same names: `ALL`, `as_str`, `FromStr`, `Display`; subsets convert with `From`/`TryFrom`          |
| `SupportedChain`, `SUPPORTED_CHAIN_MAP`, `NON_EVM_CHAIN_IDS`                         | `Chain` enum: `ALL`, `as_str` (the `data.json` key), `display_name`, `id`, `from_id`, `is_evm`, `is_testnet`   |
| `SUPPORTED_CHAIN_IDS`, `SOLANA_CHAIN_ID`, `BITCOIN_CHAIN_ID`, `SIGNET_CHAIN_ID`      | Constants of the same names (`&[u64]` and `u64`)                                                               |
| `TOKENLIST_BASE_URL`, `TOKENLIST_MIRROR_URL`, `TOKENLIST_SCHEMA_URL`                 | Constants of the same names                                                                                    |
| `TokenData` (`data.json`)                                                            | `TokenId::data()` → `TokenData`, with per-chain `ChainTokenData` and `Overrides`                               |
| `Token` (`tokenlist.json` entry)                                                     | `Token`, iterated by `tokens()`; `tokens_with_overrides()` for `tokenlist-overrides.json`                      |
| `getLogoURI`, `getMirrorLogoURI`                                                     | `logo_uri`, `mirror_logo_uri`; also `TokenId::logo_uri()`, `Token::logo_uri()`, `Chain::logo_uri()` (+ mirror) |
| `checksumAddress`, `toEvmAddress`, `toTronAddress`                                   | `checksum_address`, `to_evm_address`, `to_tron_address` (return `Result` instead of throwing)                  |
| `tokenlist*.json`, `chainlist*.json`, `supported-chains.json`, `compressedlist.json` | `json::TOKENLIST`, `json::TOKENLIST_BOB`, …, `json::COMPRESSEDLIST` (file contents, embedded verbatim)         |

Notes:

- **Variant names** keep the token ID's casing (`TokenId::cbBTC`, `TokenId::USDC`). Characters that are not valid in Rust identifiers are spelled out: `.` → `_` (`BTC.b` → `TokenId::BTC_b`) and `+` → `_PLUS` (`BTC+` → `TokenId::BTC_PLUS`). `Chain` variants are the PascalCase `data.json` key (`op-mainnet` → `Chain::OpMainnet`).
- **`#[non_exhaustive]`:** token ID enums, `Chain`, and the data structs grow as tokens and chains are added, so `match` on them needs a `_` arm.
- **Order:** `TokenId::ALL` and `tokens()` are sorted by token ID, case-insensitively by bytes, and `tokens()` then by `Chain::ALL`. The JSON lists sort with JavaScript's `localeCompare`, so a few IDs differ in position (`BTC+` / `BTC.b`); look entries up by ID rather than index.
- **CoinGecko IDs:** `Token::coingecko_id` is always set, even where `tokenlist.json` omits `extensions.coingeckoId` for IDs over the schema's 42-character cap.
- **Features:** `serde` adds `Serialize`/`Deserialize` for `Chain` (as its `data.json` key) and the token ID types (as the ID string).

```toml
gobob-tokenlist = { git = "https://github.com/bob-collective/tokenlist", rev = "<commit_hash>", features = ["serde"] }
```

The crate is self-contained Rust: its [`build.rs`](./build.rs) reads `data/tokens/*/data.json`, `data/chains/chains.json`, and `supported-chains.json` at compile time, so no TypeScript tooling is involved in building it. The TypeScript build's only contribution is JSON (`supported-chains.json` carries the chain IDs that live in `config.ts`). `cargo test` checks the crate against the TS-generated JSON files, so the npm package and the crate cannot drift apart.

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

Solana uses base58 token mint addresses and is emitted with chain ID `1584368940`. Solana addresses are passed through unchanged because they do not have EVM checksum semantics.

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
- `token-ids.json` — the same unions as JSON arrays
- `tokenlist.json` — all tokens using base names, symbols, and decimals
- `tokenlist-bob.json` — BOB chain tokens
- `tokenlist-overrides.json` — tokens with overrides applied
- `tokenlist-mirror.json`, `tokenlist-bob-mirror.json`, `tokenlist-overrides-mirror.json` — same lists with R2 mirror logo URIs
- `chainlist.json` — supported chain logo map (GitHub-hosted logos)
- `chainlist-mirror.json` — supported chain logo map (R2 mirror logos)
- `supported-chains.json` — supported chain IDs, names, EVM and testnet flags
- `compressedlist.json` — compact shared + per-chain tuple output

### 5. Verify

```bash
pnpm verify
cargo test   # Rust data matches the generated JSON files
```

---

## Scripts

| Command                     | Description                                                                 |
| --------------------------- | --------------------------------------------------------------------------- |
| `pnpm build`                | Generate types, build token lists, then run verification                    |
| `pnpm build:tokenlist`      | Generate tokenlist JSON files (GitHub-hosted and `-mirror` variants)        |
| `pnpm build:compressedlist` | Generate `compressedlist.json` (compact shared + per-chain tuples)          |
| `pnpm build:chainlist`      | Generate `chainlist.json`, `chainlist-mirror.json`, `supported-chains.json` |
| `pnpm build:types`          | Generate `TokenId` and `NativeTokenId` TypeScript unions                    |
| `pnpm check`                | Run Biome formatting, import organization, and lint checks                  |
| `pnpm check:write`          | Apply safe Biome formatting/import/lint fixes                               |
| `pnpm format`               | Check formatting with Biome                                                 |
| `pnpm format:write`         | Apply Biome formatting                                                      |
| `pnpm lint`                 | Run Biome lint rules                                                        |
| `pnpm publish:assets`       | Publish token and chain logos to Cloudflare R2 and verify the public copies |
| `pnpm verify`               | Validate tokenlist schema and on-chain token data                           |

Rust crate:

| Command                                     | Description                               |
| ------------------------------------------- | ----------------------------------------- |
| `cargo test --all-features`                 | Run Rust unit, doc, and JSON parity tests |
| `cargo fmt`                                 | Format the Rust sources                   |
| `cargo clippy --all-targets --all-features` | Lint the Rust sources                     |

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

**Generated files:** Do not edit `token-ids.ts`, `token-ids.json`, `tokenlist.json`, `tokenlist-bob.json`, `tokenlist-overrides.json`, any `-mirror.json` list, `chainlist.json`, `chainlist-mirror.json`, or `supported-chains.json` by hand. Update `data/tokens/[TOKEN]/data.json` and run `pnpm build`.

---

## Contributing

Open a PR against `bob-collective/tokenlist` with your changes.

## License

MIT
