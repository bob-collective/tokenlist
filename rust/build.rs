//! Generates the token tables from `../compressedlist.json`, the compact list
//! `pnpm build:compressedlist` writes from `data/`. Every per-chain value in it
//! is already resolved (decimals, native flag, name/symbol overrides), so this
//! script only validates the tuples and emits Rust into `$OUT_DIR/generated.rs`:
//!
//! - `TokenId`, one variant per token with at least one EVM deployment;
//! - `TOKENS`, the static `TokenMetadata` table, in `TokenId` order.
//!
//! The code is built with `quote`, parsed with `syn` and formatted with
//! `prettyplease`. Malformed or inconsistent data fails the build with the
//! offending entry in the message.

use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::path::PathBuf;
use std::{env, fs};

use heck::ToUpperCamelCase;
use quote::quote;
use serde::Deserialize;
use serde_json::Value;
use serde_with::de::DeserializeAsWrap;
use serde_with::{MapPreventDuplicates, Same};

type Result<T, E = Box<dyn Error>> = std::result::Result<T, E>;

// `TOKENLIST_BASE_URL` and `TOKENLIST_MIRROR_URL` in typescript/config.ts.
// tests/parity.rs checks the generated URLs against the JSON lists' logoURIs.
const TOKENLIST_BASE_URL: &str =
    "https://raw.githubusercontent.com/bob-collective/tokenlist/refs/heads/main/";
const TOKENLIST_MIRROR_URL: &str = "https://static.gobob.xyz/tokenlist/";

/// The EVM chains of `SUPPORTED_CHAINS` in typescript/config.ts. Every chain
/// ID in compressedlist.json must be here or in [`NON_EVM_CHAIN_IDS`].
const EVM_CHAIN_IDS: &[u64] = &[
    1,        // Ethereum
    10,       // OP Mainnet
    40,       // Telos
    56,       // BNB Smart Chain
    130,      // Unichain
    137,      // Polygon
    146,      // Sonic
    999,      // HyperEVM
    1329,     // Sei Network
    1868,     // Soneium Mainnet
    4663,     // Robinhood Chain
    8453,     // Base
    9745,     // Plasma
    42161,    // Arbitrum One
    43114,    // Avalanche
    60808,    // BOB
    80094,    // Berachain
    808813,   // BOB Sepolia
    11155111, // Sepolia
];

/// Chains without 20-byte hex addresses (`NON_EVM_CHAIN_IDS` in config.ts).
/// Their entries are left out.
const NON_EVM_CHAIN_IDS: &[u64] = &[
    728126428,  // Tron
    1231006505, // Bitcoin
    1584368940, // Solana
    1598918400, // Bitcoin signet
];

/// `[name, symbol, coingeckoId, logo]`, the first element of every entry.
#[derive(Deserialize)]
struct Shared(String, String, Option<String>, LogoExt);

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum LogoExt {
    Svg,
    Webp,
}

impl LogoExt {
    fn as_str(self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Webp => "webp",
        }
    }
}

/// `[chainId, address, decimals, native, nameOverride?, symbolOverride?]`, one
/// per chain. The name slot holds `null` when only the symbol is overridden.
#[derive(Deserialize)]
struct ChainEntry(
    u64,
    String,
    u8,
    bool,
    #[serde(default)] Option<String>,
    #[serde(default)] Option<String>,
);

fn main() -> Result<()> {
    let path = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("../compressedlist.json");
    println!("cargo::rerun-if-changed={}", path.display());

    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let code = parse(&text)
        .and_then(|list| generate(&list))
        .map_err(|e| format!("compressedlist.json: {e}"))?;

    fs::write(
        PathBuf::from(env::var("OUT_DIR")?).join("generated.rs"),
        code,
    )?;
    Ok(())
}

/// Token ID → `[shared, ...chains]`, rejecting duplicate token IDs.
fn parse(text: &str) -> Result<BTreeMap<String, Vec<Value>>> {
    let mut de = serde_json::Deserializer::from_str(text);
    let list =
        DeserializeAsWrap::<BTreeMap<String, Vec<Value>>, MapPreventDuplicates<Same, Same>>::deserialize(
            &mut de,
        )?
        .into_inner();
    de.end()?;
    Ok(list)
}

fn generate(list: &BTreeMap<String, Vec<Value>>) -> Result<String> {
    let mut variants = Vec::new();
    let mut records = Vec::new();
    // Variant name → token ID, to reject IDs that map to the same variant.
    let mut variant_ids = HashMap::<String, &str>::new();
    // (chain ID, lowercase address) → token ID, to reject reused addresses.
    let mut token_by_address = HashMap::<(u64, String), &str>::new();

    for (id, entry) in list {
        let (shared, chains) = entry
            .split_first()
            .ok_or_else(|| format!("{id}: empty entry"))?;
        let Shared(name, symbol, coingecko_id, logo) =
            Shared::deserialize(shared).map_err(|e| format!("{id}[0]: {e}"))?;
        let coingecko_id = coingecko_id.ok_or_else(|| format!("{id}: missing coingeckoId"))?;

        let mut chain_ids = Vec::new();
        let mut deployments = Vec::new();

        for (index, chain) in chains.iter().enumerate() {
            let at = format!("{id}[{}]", index + 1);
            let ChainEntry(chain_id, address, decimals, native, name_override, symbol_override) =
                ChainEntry::deserialize(chain).map_err(|e| format!("{at}: {e}"))?;

            if chain_ids.contains(&chain_id) {
                return Err(format!("{at}: duplicate chain {chain_id}").into());
            }
            chain_ids.push(chain_id);

            if NON_EVM_CHAIN_IDS.contains(&chain_id) {
                continue;
            }
            if !EVM_CHAIN_IDS.contains(&chain_id) {
                return Err(format!(
                    "{at}: unknown chain {chain_id}; add it to EVM_CHAIN_IDS or NON_EVM_CHAIN_IDS in build.rs"
                )
                .into());
            }
            if !is_evm_address(&address) {
                return Err(format!("{at}: invalid EVM address {address:?}").into());
            }
            if let Some(other) =
                token_by_address.insert((chain_id, address.to_ascii_lowercase()), id)
            {
                return Err(format!(
                    "{at}: {address} on chain {chain_id} is also {other}'s address"
                )
                .into());
            }

            let name = name_override.as_deref().unwrap_or(&name);
            let symbol = symbol_override.as_deref().unwrap_or(&symbol);

            deployments.push(quote! {
                Deployment {
                    chain_id: #chain_id,
                    address: ::alloy_primitives::address!(#address),
                    decimals: #decimals,
                    native: #native,
                    name: #name,
                    symbol: #symbol,
                }
            });
        }

        // Only deployed on non-EVM chains.
        if deployments.is_empty() {
            continue;
        }

        let rust_name = id.replace('+', "_plus_").to_upper_camel_case();
        let variant: syn::Ident = syn::parse_str(&rust_name)
            .map_err(|e| format!("{id}: TokenId::{rust_name} is not a valid identifier: {e}"))?;
        if let Some(other) = variant_ids.insert(rust_name.clone(), id) {
            return Err(format!("{id} and {other} both map to TokenId::{rust_name}").into());
        }

        let doc = format!(" `{id}`: {name}.");
        variants.push(quote! {
            #[doc = #doc]
            #[strum(serialize = #id)]
            #variant
        });

        let logo_path = format!("data/tokens/{id}/logo.{}", logo.as_str());
        let icon_url = format!("{TOKENLIST_BASE_URL}{logo_path}");
        let mirror_icon_url = format!("{TOKENLIST_MIRROR_URL}{logo_path}");

        records.push(quote! {
            TokenMetadata {
                id: TokenId::#variant,
                name: #name,
                symbol: #symbol,
                coingecko_id: #coingecko_id,
                icon_url: #icon_url,
                mirror_icon_url: #mirror_icon_url,
                deployments: &[#(#deployments),*],
            }
        });
    }

    let count = records.len();
    let code = quote! {
        /// A token with at least one EVM deployment, named after its
        /// `data/tokens/` directory: `FromStr`, `Display` and
        /// [`TokenId::as_str`] use that exact ID string (`"BTC+"` is
        /// `TokenId::BtcPlus`).
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
            ::strum::EnumString,
            ::strum::IntoStaticStr,
            ::strum::Display,
        )]
        #[non_exhaustive]
        pub enum TokenId {
            #(#variants),*
        }

        /// Every token, in `TokenId` order: `TOKENS[id as usize].id == id`.
        pub(crate) static TOKENS: [TokenMetadata; #count] = [#(#records),*];
    };

    let file: syn::File = syn::parse2(code)?;
    Ok(prettyplease::unparse(&file))
}

fn is_evm_address(address: &str) -> bool {
    address
        .strip_prefix("0x")
        .is_some_and(|hex| hex.len() == 40 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}
