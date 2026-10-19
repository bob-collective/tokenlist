# AGENTS.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
pnpm build              # Full build: generates types, tokenlist JSONs, then verifies
pnpm build:tokenlist    # Generate tokenlist JSON files (+ -mirror variants) from /data
pnpm build:chainlist    # Generate chainlist.json, chainlist-mirror.json, supported-chains.json
pnpm build:types        # Regenerate token-ids.ts from current /data entries
pnpm check              # Run Biome formatting, import organization, and lint checks
pnpm check:write        # Apply safe Biome formatting/import/lint fixes
pnpm format             # Check formatting with Biome
pnpm format:write       # Apply Biome formatting
pnpm lint               # Run Biome lint rules
pnpm verify             # Validate schema + on-chain data (runs automatically after build)

cargo test --all-features                                  # Rust unit, doc, and JSON parity tests
cargo fmt                                                  # Format Rust sources
cargo clippy --all-targets --all-features -- -D warnings   # Lint Rust sources (as CI does)
```

## Architecture

This package is the authoritative token registry for the BOB ecosystem. It ships two ways from the same data: the `@gobob/tokenlist` npm package (TypeScript + JSON) and the `gobob-tokenlist` Rust crate (root `Cargo.toml`, installed as a Git dependency). The source of truth is the `/data` directory, which contains:

- `data/tokens/` — one subdirectory per token (named by symbol), each containing a `data.json` and a logo asset
- `data/chains/` — chain registry; `chains.json` maps each supported chain name to a logoURI

**Data flow:**

```
data/tokens/[TOKEN]/data.json  →  scripts/build-tokenlist.ts  →  tokenlist.json
                                                               →  tokenlist-bob.json
                                                               →  tokenlist-overrides.json
                                                               →  tokenlist-mirror.json
                                                               →  tokenlist-bob-mirror.json
                                                               →  tokenlist-overrides-mirror.json
data/tokens/*/                 →  scripts/build-types.ts      →  token-ids.ts (auto-generated)
data/chains/chains.json        →  scripts/build-chainlist.ts   →  chainlist.json
  + config.ts                                                  →  chainlist-mirror.json
                                                               →  supported-chains.json

data/tokens/*/                 →  build.rs (Rust, at compile time)  →  Rust token/chain tables
  + data/chains/chains.json
  + supported-chains.json
```

TypeScript and Rust share nothing but JSON: `scripts/` never writes Rust, and the crate never reads `.ts` files. Chain metadata that only exists in `config.ts` (viem chain IDs, testnet flags) reaches Rust through `supported-chains.json`.

**Key files:**

- `config.ts` — Chain definitions, supported chain list, file path constants, and GitHub logo URI base. All chain-to-chainId mappings live here.
- `types.ts` — `TokenData` (raw input shape from data.json), `Token` (output shape for tokenlist JSONs), `SupportedChain` type.
- `index.ts` — Public API surface; re-exports from config, types, token-ids, utils.
- `token-ids.ts` — Auto-generated union type of all token symbols. Never edit by hand; run `pnpm build:types`.
- `token.schema.json` — JSON Schema attached to `data/tokens/[TOKEN]/data.json` files for IDE autocompletion.
- `chain.schema.json` — JSON Schema attached to `data/chains/chains.json` for IDE autocompletion.
- `Cargo.toml`, `src/`, `tests/` — the Rust crate; see **Rust crate** below.

**data.json structure:**

Each token directory's `data.json` specifies:

- `name`, `symbol`, `decimals`
- `coingeckoId` — required CoinGecko API ID for price feeds, emitted as `extensions.coingeckoId` in output JSONs; receipt/wrapped tokens without their own listing use the underlying asset's ID
- `tokens` — map of chain name (e.g. `"bob"`, `"ethereum"`, `"op-mainnet"`, `"solana"`) → per-chain token record
- `tokens[chain].address` — token contract address or Solana mint; native EVM assets use `0x0000000000000000000000000000000000000000`
- `tokens[chain].bridge` — map of related chain name → bridge token address
- `tokens[chain].overrides` — per-chain UI overrides (e.g. rename symbol to `"USDC.e"` on BOB)
- Optional metadata: `website`, `twitter`, `description`

Chain names used as keys in `tokens`, `bridge`, and `overrides` must match the supported chain keys in `config.ts`/`token.schema.json`. Chain IDs are only used in the output JSONs.

Non-EVM handling:

- Solana has no viem chain definition; `config.ts` maps `solana` to chain ID `1584368940`, and Solana mint addresses are passed through without EVM checksum formatting.

**Output JSONs:**

- `tokenlist.json` — All tokens across all chains, flat format
- `tokenlist-bob.json` — BOB-chain entries only
- `tokenlist-overrides.json` — Full list with per-chain overrides applied
- `tokenlist*-mirror.json` — Same three lists with `logoURI` pointing at the R2 mirror (`static.gobob.xyz`) instead of GitHub raw
- `chainlist.json` — Chain ID to logo URI map (GitHub-hosted logos)
- `chainlist-mirror.json` — Chain ID to logo URI map (R2 mirror logos, `static.gobob.xyz`)
- `supported-chains.json` — Every supported chain keyed by its `data.json` name: `chainId`, display `name`, `evm`, `testnet`, in `SUPPORTED_CHAIN_IDS` order

These are committed to the repo and consumed downstream via the npm package; the Rust crate embeds them verbatim.

**Rust crate:**

The repo root is also the `gobob-tokenlist` crate (`publish = false`; consumers depend on it via `git = "https://github.com/bob-collective/tokenlist"`). It mirrors the npm API: token ID enums, `Chain`, `TokenData`/`Token`, logo URI and address helpers, and the JSON files embedded verbatim in `src/json.rs`.

- `build.rs` — Reads `data/tokens/*/data.json` (+ logo extension), `data/chains/chains.json`, and `supported-chains.json`, and writes `$OUT_DIR/generated.rs` (`define_chains!`, `define_token_ids!`, `define_token_id_subset!` invocations and the static `TOKENS` table), included by `src/lib.rs`. Nothing generated is committed. Addresses are checksummed with `src/address.rs`, shared via `#[path]`. Malformed data fails the build with the offending file in the message.
- `src/macros.rs` — All behaviour of the generated enums (`as_str`, `FromStr`, `From`/`TryFrom`, serde). `build.rs` emits data only; put logic here or in the other modules, not in the generator's output.
- `src/token.rs` — `TokenData`, `ChainTokenData`, `Overrides`, `Token`, `TokenId::data()`, `tokens()`, `tokens_with_overrides()`. Name/symbol/decimals resolution must match `scripts/build-tokenlist.ts`.
- `src/lib.rs` — `TOKENLIST_SCHEMA_URL`, `TOKENLIST_BASE_URL`, `TOKENLIST_MIRROR_URL` (Rust copies of the `config.ts` constants; `tests/parity.rs` checks them against the JSON lists).
- `src/logo.rs`, `src/address.rs` — Rust equivalents of `getLogoURI`/`getMirrorLogoURI` and `checksumAddress`/`toEvmAddress`/`toTronAddress`. Address tests use vectors produced by the npm package.
- `tests/parity.rs` — Compares the Rust data (built from `data/`) against every TS-generated JSON file, entry by entry keyed on `(tokenId, chainId)`, and checks `Cargo.toml` `version` equals the `version` in `tokenlist.json`. Rust sorts token IDs by lowercase bytes; the JSON lists use JavaScript `localeCompare`, so their order differs (e.g. `BTC+` / `BTC.b`).

Rules:

- Never generate Rust from TypeScript or read `.ts` files from Rust. If the crate needs data that only TS knows, emit it as JSON from a `scripts/` build step and read that JSON in `build.rs`.
- Keep the public APIs aligned: a consumer-facing export in `config.ts`, `types.ts`, or `utils.ts` should get a Rust counterpart (and a README "npm ↔ Rust" row).
- Bump `version` in `Cargo.toml` and `package.json` together.
- The Rust `Chain` variant is the PascalCase `supported-chains.json` key (`op-mainnet` → `OpMainnet`). A new non-EVM testnet must be added to `NON_EVM_TESTNET_CHAIN_NAMES` in `config.ts` so `supported-chains.json` flags it.
- Token ID variants keep the directory name's casing; `.` → `_` and `+` → `_PLUS`. `build.rs` fails on any other character or on an identifier collision — extend `token_ident` there if a new token needs it.
- Keep dependency requirements loose (`"0.11"`, `"1"`, ranges where APIs allow) so consumers reuse the versions in their `Cargo.lock`, and enable no features on build-dependencies: Cargo unifies them with every proc macro and build script in a consumer's graph (e.g. `preserve_order` on `serde_json` would change typify/progenitor codegen). `build.rs` reads ordered JSON objects with its `Entries` visitor instead.
- The crate's MSRV is 1.88 (`rust-version`, set by `sha2`); change it in `Cargo.toml` and the `msrv` job of `.github/workflows/rust.yml` together.
- CI (`.github/workflows/rust.yml`) runs `cargo fmt --check`, `cargo clippy -D warnings`, and `cargo test` with and without `--all-features`, and checks the MSRV. It needs no Node toolchain; `cargo test` fails if `data/` changed without `pnpm build`.

**Adding a new token:**

1. Create `data/tokens/[SYMBOL]/data.json` (use `token.schema.json` for structure)
2. Add a logo asset (`logo.svg` or `logo.webp`)
3. Run `pnpm build` — this regenerates `token-ids.ts`, all JSON outputs, and runs on-chain verification
4. Run `cargo test` — the crate picks up `data/` at compile time; this confirms it matches the regenerated JSON files

**Sourcing logos (tokens and chains):**

Prefer an official SVG from the issuer's own site — the company/foundation that issues the token or runs the chain. Look for a brand kit / press kit / media kit page (often linked in the footer as "Brand", "Press", "Media", or "Assets"), or the SVG logo embedded in the site header.

Examples of issuer sites:

- USDT → https://tether.to/en/
- USDC → https://www.circle.com

Guidelines:

- Use the token/chain mark itself (the coin icon), not a wordmark or the parent company logo when they differ.
- SVG first; fall back to `logo.webp` only if the issuer publishes no SVG.
- Avoid aggregators (CoinGecko, CoinMarketCap, Trust Wallet assets, etc.) as the primary source; use them only when the issuer site has nothing usable, and note it in the PR.
- Remove `width` and `height` attributes from the root `<svg>` and keep `viewBox` (add one if missing, e.g. `viewBox="0 0 W H"` from the removed width/height) so logos scale correctly to any size.
- Token logos go in `data/tokens/[SYMBOL]/logo.svg`; chain logos go in `data/chains/[name].svg` and are referenced from `data/chains/chains.json`.
