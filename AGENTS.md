# AGENTS.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
pnpm build                # Full build: generates types, tokenlist JSONs, then verifies
pnpm build:tokenlist      # Generate tokenlist JSON files (+ -mirror variants) from /data
pnpm build:chainlist      # Generate chainlist.json, chainlist-mirror.json, supported-chains.json
pnpm build:compressedlist # Generate compressedlist.json (the Rust crate's input) from /data
pnpm build:types          # Regenerate typescript/token-ids.ts from current /data entries
pnpm check                # Run Biome formatting, import organization, and lint checks
pnpm check:write          # Apply safe Biome formatting/import/lint fixes
pnpm format               # Check formatting with Biome
pnpm format:write         # Apply Biome formatting
pnpm lint                 # Run Biome lint rules
pnpm verify               # Validate schema + on-chain data (runs automatically after build)

# Rust crate — run inside rust/
cargo test --all-features                                  # Unit, doc, and JSON parity tests
cargo fmt                                                  # Format Rust sources
cargo clippy --all-targets --all-features -- -D warnings   # Lint Rust sources (as CI does)
```

## Architecture

This repository is the authoritative token registry for the BOB ecosystem. It ships two ways from the same data: the `@gobob/tokenlist` npm package (TypeScript + JSON) and the `tokenlist` Rust crate (installed as a Git dependency). The source of truth is the `/data` directory, which contains:

- `data/tokens/` — one subdirectory per token (named by symbol), each containing a `data.json` and a logo asset
- `data/chains/` — chain registry; `chains.json` maps each supported chain name to a logoURI

**Repository layout:**

- `data/`, the generated JSON files, and `token.schema.json`/`chain.schema.json` stay at the root and are shared by both packages. Raw GitHub URLs (every `logoURI`, consumers fetching `tokenlist.json`) and npm subpath imports (`@gobob/tokenlist/tokenlist.json`) depend on these paths; never move them.
- `typescript/` — the npm package sources (`index.ts`, `config.ts`, `types.ts`, `utils.ts`, `token-ids.ts`), the build scripts (`typescript/scripts/`), and `tsconfig.json`.
- `rust/` — the Rust crate (`Cargo.toml`, `build.rs`, `src/`, `tests/`).
- `package.json` stays at the root: installs from a GitHub URL need it there. Its `main`/`exports` point into `typescript/`, so `@gobob/tokenlist` and `@gobob/tokenlist/<module>` resolve as before; `typesVersions` does the same for TypeScript's legacy `node` resolution. A new public module needs an entry in `files` (the `exports` patterns already cover it). JSON subpaths resolve only with their `.json` extension.

Scripts run with the repo root as working directory, so the path constants in `typescript/config.ts` (`TOKEN_DIR`, `OUTFILE_*`) are root-relative; `build-types.ts` writes `OUTFILE_TYPES` next to `config.ts`.

**Data flow:**

```
data/tokens/[TOKEN]/data.json  →  typescript/scripts/build-tokenlist.ts       →  tokenlist.json
                                                                              →  tokenlist-bob.json
                                                                              →  tokenlist-overrides.json
                                                                              →  tokenlist-mirror.json
                                                                              →  tokenlist-bob-mirror.json
                                                                              →  tokenlist-overrides-mirror.json
data/tokens/*/                 →  typescript/scripts/build-compressedlist.ts  →  compressedlist.json
data/tokens/*/                 →  typescript/scripts/build-types.ts           →  typescript/token-ids.ts (auto-generated)
data/chains/chains.json        →  typescript/scripts/build-chainlist.ts       →  chainlist.json
  + typescript/config.ts                                                      →  chainlist-mirror.json
                                                                              →  supported-chains.json

supported-chains.json          →  rust/build.rs (at compile time)             →  ChainId enum
compressedlist.json            →  rust/build.rs (at compile time)             →  TokenId enum + static token table
```

**Key files:**

- `typescript/config.ts` — Chain definitions, supported chain list, file path constants, and GitHub logo URI base. All chain-to-chainId mappings live here.
- `typescript/types.ts` — `TokenData` (raw input shape from data.json), `Token` (output shape for tokenlist JSONs), `SupportedChain` type.
- `typescript/index.ts` — Public API surface; re-exports from config, types, token-ids, utils.
- `typescript/token-ids.ts` — Auto-generated union type of all token symbols. Never edit by hand; run `pnpm build:types`.
- `token.schema.json` — JSON Schema attached to `data/tokens/[TOKEN]/data.json` files for IDE autocompletion.
- `chain.schema.json` — JSON Schema attached to `data/chains/chains.json` for IDE autocompletion.

**data.json structure:**

Each token directory's `data.json` specifies:

- `name`, `symbol`, `decimals`
- `coingeckoId` — required CoinGecko API ID for price feeds, emitted as `extensions.coingeckoId` in output JSONs; receipt/wrapped tokens without their own listing use the underlying asset's ID
- `tokens` — map of chain name (e.g. `"bob"`, `"ethereum"`, `"op-mainnet"`, `"solana"`) → per-chain token record
- `tokens[chain].address` — token contract address or Solana mint; native EVM assets use `0x0000000000000000000000000000000000000000`
- `tokens[chain].bridge` — map of related chain name → bridge token address
- `tokens[chain].overrides` — per-chain UI overrides (e.g. rename symbol to `"USDC.e"` on BOB)
- Optional metadata: `website`, `twitter`, `description`

Chain names used as keys in `tokens`, `bridge`, and `overrides` must match the supported chain keys in `typescript/config.ts`/`token.schema.json`. Chain IDs are only used in the output JSONs.

Non-EVM handling:

- Solana has no viem chain definition; `typescript/config.ts` maps `solana` to chain ID `1584368940`, and Solana mint addresses are passed through without EVM checksum formatting.

**Output JSONs:**

- `tokenlist.json` — All tokens across all chains, flat format
- `tokenlist-bob.json` — BOB-chain entries only
- `tokenlist-overrides.json` — Full list with per-chain overrides applied
- `tokenlist*-mirror.json` — Same three lists with `logoURI` pointing at the R2 mirror (`static.gobob.xyz`) instead of GitHub raw
- `chainlist.json` — Chain ID to logo URI map (GitHub-hosted logos)
- `chainlist-mirror.json` — Chain ID to logo URI map (R2 mirror logos, `static.gobob.xyz`)
- `supported-chains.json` — Every supported chain keyed by its `data.json` name: `chainId` and `evm` (false for the chains in `NON_EVM_CHAIN_NAMES`). The Rust crate's `ChainId` source
- `compressedlist.json` — Compact `[shared, ...chains]` tuples per token, with per-chain decimals, native flag, and name/symbol overrides already resolved; the Rust crate's token source

These are committed to the repo and consumed downstream via the npm package.

**Rust crate (`rust/`):**

`tokenlist` (`publish = false`; consumers depend on it with `git = "https://github.com/bob-collective/tokenlist"`, and Cargo finds it in `rust/`). It covers EVM, Bitcoin, Solana and Tron deployments; Signet remains unsupported. Bitcoin uses chain ID `90000` and the zero-address placeholder.

- `rust/build.rs` — Reads `../supported-chains.json` and `../compressedlist.json` (nothing else), validates them, builds the code with `quote`, parses it with `syn`, formats it with `prettyplease`, and writes `$OUT_DIR/generated.rs`: the `ChainId` enum (EVM, Bitcoin, Solana and Tron chains), the `TokenId` enum (tokens with a supported deployment), a flat static `TokenData` array, a `u16` token/chain index (zero means absent, otherwise record index plus one), numeric chain IDs, and `ALL` variant lists. Enum ordinals are dense internal indices, separate from numeric network IDs. Index overflow fails the build. Both enums derive strum `EnumString`/`IntoStaticStr`/`Display`. Nothing generated is committed. The build fails, naming the entry, on duplicate keys (`serde_with::MapPreventDuplicates`), keys that map to the same variant or to no valid identifier, a chain ID listed twice, duplicate chains within a token, an address reused on a chain, malformed tuples or addresses, and chain IDs missing from `supported-chains.json`.
- `rust/src/lib.rs` — `TokenData` (one token on one chain; `TokenMetadata` remains a type alias), `TokenAddress` (`Evm(Address)`, `Bitcoin(Address)`, `Solana(&'static str)`, `Tron(&'static str)`; `Copy` and `Display`; base58 validation uses `bs58` at build time), `TokenRegistry::get(id, chain)`, `TokenId::on_chain(chain)`, and `ChainId::get(id)` return `Option<TokenData>`. `get_ref` and the compatible registry `on_chain` return borrowed metadata; `all` returns a flat deployment slice. All pair lookups use the static index: worst-case O(1), no runtime initialization, allocations or hashing. Both enums expose `as_str`; `ChainId::chain_id` returns the numeric network ID. Behaviour belongs here; `build.rs` emits data and enum definitions only.
- `rust/tests/parity.rs` — Checks the generated tables against `supported-chains.json`, `compressedlist.json` (both parsed independently), and `tokenlist-overrides.json` entry by entry, each deployment's `ChainId` key against its `chain_id`, and `Cargo.toml` `version` against `tokenlist.json` and `package.json`. `rust/tests/registry.rs` exhaustively checks every token/chain pair. `rust/tests/generator.rs` imports the build script to test validation and index capacity.

Rules:

- TypeScript and Rust share nothing but JSON: the crate never reads `.ts` files or `data/`, and `typescript/scripts/` never writes Rust. Do not change the `compressedlist.json` tuple format for Rust's sake; npm consumers parse it too.
- Variants: `key.replace('+', "_plus_").to_upper_camel_case()` for token IDs (`BTC+` → `BtcPlus`, `BTC.b` → `BtcB`) and chain keys (`op-mainnet` → `OpMainnet`); `FromStr`/`Display`/`as_str` use the original string.
- Bump `version` in `rust/Cargo.toml` and `package.json` together.
- Keep dependency requirements loose (`"1"`, ranges where APIs allow) so consumers reuse the versions in their `Cargo.lock`. Build-dependency features are unified with every proc macro and build script in a consumer's graph, so enable none that change behaviour (e.g. `preserve_order` on `serde_json`).
- The crate's MSRV is 1.88 (`rust-version`, set by `serde_with`); change it in `rust/Cargo.toml` and the `msrv` job of `.github/workflows/rust.yml` together.
- CI: `.github/workflows/rust.yml` runs `cargo fmt --check`, `cargo clippy -D warnings`, and `cargo test` with and without `--all-features` in `rust/`, plus the MSRV check; it needs no Node toolchain. `.github/workflows/biome.yml` regenerates `typescript/token-ids.ts`, `chainlist*.json`, `supported-chains.json`, and `compressedlist.json` and fails if they differ from the committed files, so `data/` edits cannot skip `pnpm build`.

**Adding a new token:**

1. Create `data/tokens/[SYMBOL]/data.json` (use `token.schema.json` for structure)
2. Add a logo asset (`logo.svg` or `logo.webp`)
3. Run `pnpm build` — this regenerates `typescript/token-ids.ts`, all JSON outputs, and runs on-chain verification
4. Run `cargo test` in `rust/` — the crate picks up `supported-chains.json` and `compressedlist.json` at compile time; this confirms it matches the regenerated JSON files

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
