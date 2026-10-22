//! Generates `$OUT_DIR/generated.rs` from two files the npm build writes:
//!
//! - `../supported-chains.json`: `ChainId`, one variant per EVM chain;
//! - `../compressedlist.json`: `TokenId`, one variant per token with an EVM
//!   deployment, and `tokens()`, every token's `TokenMetadata` keyed by
//!   `TokenId`, then by `ChainId`.
//!
//! The code is built with `quote`, parsed with `syn` and formatted with
//! `prettyplease`. Malformed or inconsistent data fails the build with the
//! offending entry in the message.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::{env, fs};

use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::quote;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use serde_with::de::DeserializeAsWrap;
use serde_with::{MapPreventDuplicates, Same};

type Result<T, E = Box<dyn Error>> = std::result::Result<T, E>;

// `TOKENLIST_BASE_URL` in typescript/config.ts; tests/parity.rs checks the
// generated URLs against the logoURIs in tokenlist-overrides.json.
const TOKENLIST_BASE_URL: &str =
    "https://raw.githubusercontent.com/bob-collective/tokenlist/refs/heads/main/";

/// A `supported-chains.json` entry, keyed by the chain's `data.json` name.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SupportedChain {
    chain_id: u64,
    evm: bool,
}

/// `[name, symbol, coingeckoId, logo]`, the first element of every
/// `compressedlist.json` entry.
#[derive(Deserialize)]
struct Shared(String, String, Option<String>, String);

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
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("..");
    let chains = read_map(&root, "supported-chains.json")?;
    let tokens = read_map(&root, "compressedlist.json")?;

    let (chain_code, chain_variants) =
        generate_chains(&chains).map_err(|e| format!("supported-chains.json: {e}"))?;
    let token_code = generate_tokens(&tokens, &chain_variants)
        .map_err(|e| format!("compressedlist.json: {e}"))?;

    let file: syn::File = syn::parse2(quote!(#chain_code #token_code))?;
    let out = PathBuf::from(env::var("OUT_DIR")?).join("generated.rs");
    fs::write(out, prettyplease::unparse(&file))?;
    Ok(())
}

/// A JSON object at the repo root, by key, rejecting duplicate keys.
fn read_map<T: DeserializeOwned>(root: &Path, file: &str) -> Result<BTreeMap<String, T>> {
    let path = root.join(file);
    println!("cargo::rerun-if-changed={}", path.display());

    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut de = serde_json::Deserializer::from_str(&text);
    DeserializeAsWrap::<BTreeMap<String, T>, MapPreventDuplicates<Same, Same>>::deserialize(&mut de)
        .and_then(|map| de.end().map(|()| map.into_inner()))
        .map_err(|e| format!("{file}: {e}").into())
}

/// The enum variant for a token ID or chain key: `+` becomes `_plus_`, then
/// UpperCamelCase. Rejects invalid identifiers and two keys sharing a variant.
fn variant(enum_name: &str, key: &str, keys: &mut HashMap<String, String>) -> Result<syn::Ident> {
    let name = key.replace('+', "_plus_").to_upper_camel_case();
    let ident = syn::parse_str(&name)
        .map_err(|e| format!("{key}: {enum_name}::{name} is not a valid identifier: {e}"))?;
    if let Some(other) = keys.insert(name.clone(), key.to_owned()) {
        return Err(format!("{key} and {other} both map to {enum_name}::{name}").into());
    }
    Ok(ident)
}

/// The `ChainId` enum, and each chain ID's variant (`None` for non-EVM chains,
/// whose deployments are left out).
fn generate_chains(
    chains: &BTreeMap<String, SupportedChain>,
) -> Result<(TokenStream, HashMap<u64, Option<syn::Ident>>)> {
    let mut variants = Vec::new();
    let mut chain_variants = HashMap::new();
    let mut keys = HashMap::new();

    for (key, chain) in chains {
        let id = chain.chain_id;
        let variant = chain
            .evm
            .then(|| variant("ChainId", key, &mut keys))
            .transpose()?;
        if let Some(variant) = &variant {
            let doc = format!(" `{key}` (chain ID {id}).");
            variants.push(quote!(#[doc = #doc] #[strum(serialize = #key)] #variant));
        }
        if chain_variants.insert(id, variant).is_some() {
            return Err(format!("{key}: chain ID {id} is listed twice").into());
        }
    }

    let code = quote! {
        /// An EVM chain, named after its `data.json` key: `FromStr`, `Display`
        /// and [`ChainId::as_str`] use that key (`"op-mainnet"` is
        /// `ChainId::OpMainnet`).
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
            ::strum::EnumString, ::strum::IntoStaticStr, ::strum::Display,
        )]
        #[non_exhaustive]
        pub enum ChainId { #(#variants),* }
    };
    Ok((code, chain_variants))
}

fn generate_tokens(
    tokens: &BTreeMap<String, Vec<Value>>,
    chain_variants: &HashMap<u64, Option<syn::Ident>>,
) -> Result<TokenStream> {
    let mut variants = Vec::new();
    let mut records = Vec::new();
    let mut keys = HashMap::new();
    // (chain ID, lowercase address), to reject addresses used by two tokens.
    let mut addresses = HashSet::new();

    for (id, entry) in tokens {
        let (shared, chains) = entry.split_first().ok_or(format!("{id}: empty entry"))?;
        let Shared(name, symbol, coingecko_id, logo) =
            Shared::deserialize(shared).map_err(|e| format!("{id}[0]: {e}"))?;
        let coingecko_id = coingecko_id.ok_or(format!("{id}: missing coingeckoId"))?;
        if logo != "svg" && logo != "webp" {
            return Err(format!("{id}[0]: unknown logo format {logo:?}").into());
        }

        let mut seen_chains = HashSet::new();
        // (ChainId variant, per-chain fields); the TokenId variant is only
        // made once the token is known to have an EVM deployment.
        let mut deployments = Vec::new();

        for (index, chain) in chains.iter().enumerate() {
            let at = format!("{id}[{}]", index + 1);
            let ChainEntry(chain_id, address, decimals, native, name_override, symbol_override) =
                ChainEntry::deserialize(chain).map_err(|e| format!("{at}: {e}"))?;

            if !seen_chains.insert(chain_id) {
                return Err(format!("{at}: duplicate chain {chain_id}").into());
            }
            let Some(chain_variant) = chain_variants.get(&chain_id) else {
                return Err(format!(
                    "{at}: chain {chain_id} is not in supported-chains.json; add it to typescript/config.ts and run `pnpm build`"
                )
                .into());
            };
            let Some(chain_variant) = chain_variant else {
                continue;
            };
            let valid_hex = address
                .strip_prefix("0x")
                .is_some_and(|hex| hex.len() == 40 && hex.bytes().all(|b| b.is_ascii_hexdigit()));
            if !valid_hex {
                return Err(format!("{at}: invalid EVM address {address:?}").into());
            }
            if !addresses.insert((chain_id, address.to_ascii_lowercase())) {
                return Err(
                    format!("{at}: another token uses {address} on chain {chain_id}").into(),
                );
            }

            let name = name_override.as_deref().unwrap_or(&name);
            let symbol = symbol_override.as_deref().unwrap_or(&symbol);
            deployments.push((
                chain_variant,
                quote! {
                    chain_id: #chain_id,
                    address: ::alloy_primitives::address!(#address),
                    decimals: #decimals,
                    native: #native,
                    name: #name,
                    symbol: #symbol,
                },
            ));
        }

        // Only deployed on non-EVM chains.
        if deployments.is_empty() {
            continue;
        }

        let variant = variant("TokenId", id, &mut keys)?;
        let doc = format!(" `{id}`: {name}.");
        variants.push(quote!(#[doc = #doc] #[strum(serialize = #id)] #variant));

        let icon_url = format!("{TOKENLIST_BASE_URL}data/tokens/{id}/logo.{logo}");
        let deployments = deployments.iter().map(|(chain_variant, fields)| {
            quote! {
                (ChainId::#chain_variant, TokenMetadata {
                    id: TokenId::#variant,
                    #fields
                    coingecko_id: #coingecko_id,
                    icon_url: #icon_url,
                })
            }
        });
        records.push(quote! {
            (TokenId::#variant, ::std::collections::HashMap::from([#(#deployments),*]))
        });
    }

    Ok(quote! {
        /// A token with at least one EVM deployment, named after its
        /// `data/tokens/` directory: `FromStr`, `Display` and
        /// [`TokenId::as_str`] use that exact ID string (`"BTC+"` is
        /// `TokenId::BtcPlus`).
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
            ::strum::EnumString, ::strum::IntoStaticStr, ::strum::Display,
        )]
        #[non_exhaustive]
        pub enum TokenId { #(#variants),* }

        /// Every token, keyed by `TokenId`, then by `ChainId`.
        pub(crate) fn tokens() -> ::std::collections::HashMap<
            TokenId,
            ::std::collections::HashMap<ChainId, TokenMetadata>,
        > {
            ::std::collections::HashMap::from([#(#records),*])
        }
    })
}
