# AGENTS.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
pnpm build              # Full build: generates types, tokenlist JSONs, then verifies
pnpm build:tokenlist    # Generate tokenlist JSON files (+ -mirror variants) from /data
pnpm build:chainlist    # Generate chainlist.json + chainlist-mirror.json from /data/chains
pnpm build:types        # Regenerate src/token-ids.ts from current /data entries
pnpm check              # Run Biome formatting, import organization, and lint checks
pnpm check:write        # Apply safe Biome formatting/import/lint fixes
pnpm format             # Check formatting with Biome
pnpm format:write       # Apply Biome formatting
pnpm lint               # Run Biome lint rules
pnpm verify             # Validate schema + on-chain data (runs automatically after build)
```

## Architecture

This package is the authoritative token registry for the BOB ecosystem. The source of truth is the `/data` directory, which contains:

- `data/tokens/` — one subdirectory per token (named by symbol), each containing a `data.json` and a logo asset
- `data/chains/` — chain registry; `chains.json` maps each supported chain name to a logoURI

**Repository layout:**

- `src/` — the npm package sources (`index.ts`, `config.ts`, `types.ts`, `utils.ts`, `token-ids.ts`).
- `scripts/` — build, verification, and asset publishing scripts; not shipped in the npm package.
- `data/`, the generated JSON files, and the JSON schemas stay at the root. Raw GitHub URLs (every `logoURI`) and npm subpath imports (`@gobob/tokenlist/tokenlist.json`) depend on these paths; never move them.
- `package.json` stays at the root: installs from a GitHub URL need it there. Its `main`/`exports` point into `src/`, so `@gobob/tokenlist` and `@gobob/tokenlist/<module>` resolve as before; `typesVersions` does the same for TypeScript's legacy `node` resolution. A new public module needs an entry in `files`. JSON subpaths resolve only with their `.json` extension.

Scripts run with the repo root as working directory, so the path constants in `src/config.ts` (`TOKEN_DIR`, `OUTFILE_*`) are root-relative.

**Data flow:**

```
data/tokens/[TOKEN]/data.json  →  scripts/build-tokenlist.ts  →  tokenlist.json
                                                               →  tokenlist-bob.json
                                                               →  tokenlist-overrides.json
                                                               →  tokenlist-mirror.json
                                                               →  tokenlist-bob-mirror.json
                                                               →  tokenlist-overrides-mirror.json
data/tokens/*/                 →  scripts/build-types.ts      →  src/token-ids.ts (auto-generated)
data/chains/chains.json        →  scripts/build-chainlist.ts   →  chainlist.json
                                                               →  chainlist-mirror.json
```

**Key files:**

- `src/config.ts` — Chain definitions, supported chain list, file path constants, and GitHub logo URI base. All chain-to-chainId mappings live here.
- `src/types.ts` — `TokenData` (raw input shape from data.json), `Token` (output shape for tokenlist JSONs), `SupportedChain` type.
- `src/index.ts` — Public API surface; re-exports from config, types, token-ids, utils.
- `src/token-ids.ts` — Auto-generated union type of all token symbols. Never edit by hand; run `pnpm build:types`.
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

Chain names used as keys in `tokens`, `bridge`, and `overrides` must match the supported chain keys in `src/config.ts`/`token.schema.json`. Chain IDs are only used in the output JSONs.

Non-EVM handling:

- Solana has no viem chain definition; `src/config.ts` maps `solana` to chain ID `1399811149`, and Solana mint addresses are passed through without EVM checksum formatting.

**Output JSONs:**

- `tokenlist.json` — All tokens across all chains, flat format
- `tokenlist-bob.json` — BOB-chain entries only
- `tokenlist-overrides.json` — Full list with per-chain overrides applied
- `tokenlist*-mirror.json` — Same three lists with `logoURI` pointing at the R2 mirror (`static.gobob.xyz`) instead of GitHub raw
- `chainlist.json` — Chain ID to logo URI map (GitHub-hosted logos)
- `chainlist-mirror.json` — Chain ID to logo URI map (R2 mirror logos, `static.gobob.xyz`)

These are committed to the repo and consumed downstream via the npm package.

**Adding a new token:**

1. Create `data/tokens/[SYMBOL]/data.json` (use `token.schema.json` for structure)
2. Add a logo asset (`logo.svg` or `logo.webp`)
3. Run `pnpm build` — this regenerates `src/token-ids.ts`, all JSON outputs, and runs on-chain verification

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
