//! Generates `$OUT_DIR/generated.rs` from two files the npm build writes:
//!
//! - `../supported-chains.json`: `ChainId`, one variant per supported chain;
//! - `../compressedlist.json`: `TokenId`, one variant per token with a supported
//!   deployment, a flat `TokenData` array and a compact enum-indexed table.
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
use quote::{format_ident, quote};
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

// Numeric chain ID → (dense enum ordinal, address kind), or None if unsupported.
type ChainVariants = HashMap<u64, Option<(usize, &'static str)>>;

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
    parse_map(&text).map_err(|e| format!("{file}: {e}").into())
}

fn parse_map<T: DeserializeOwned>(text: &str) -> Result<BTreeMap<String, T>> {
    let mut de = serde_json::Deserializer::from_str(text);
    DeserializeAsWrap::<BTreeMap<String, T>, MapPreventDuplicates<Same, Same>>::deserialize(&mut de)
        .and_then(|map| de.end().map(|()| map.into_inner()))
        .map_err(Into::into)
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

/// The `ChainId` enum, and each chain ID's ordinal (`None` for unsupported chains,
/// whose deployments are left out).
fn generate_chains(
    chains: &BTreeMap<String, SupportedChain>,
) -> Result<(TokenStream, ChainVariants)> {
    let mut variants = Vec::new();
    let mut names = Vec::new();
    let mut chain_ids = Vec::new();
    let mut chain_variants = HashMap::new();
    let mut keys = HashMap::new();

    for (key, chain) in chains {
        let id = chain.chain_id;
        let kind = match (chain.evm, key.as_str()) {
            (true, _) => Some("Evm"),
            (_, "solana") => Some("Solana"),
            (_, "tron") => Some("Tron"),
            _ => None,
        };
        let variant = kind
            .map(|_| variant("ChainId", key, &mut keys))
            .transpose()?;
        if let Some(variant) = &variant {
            let doc = format!(" `{key}` (chain ID {id}).");
            variants.push(quote!(#[doc = #doc] #[strum(serialize = #key)] #variant));
            names.push(variant.clone());
            chain_ids.push(id);
        }
        let variant = kind.map(|kind| (names.len() - 1, kind));
        if chain_variants.insert(id, variant).is_some() {
            return Err(format!("{key}: chain ID {id} is listed twice").into());
        }
    }

    let count = names.len();
    let code = quote! {
        /// A supported chain, named after its `data.json` key: `FromStr`, `Display`
        /// and [`ChainId::as_str`] use that key (`"op-mainnet"` is
        /// `ChainId::OpMainnet`).
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
            ::strum::EnumString, ::strum::IntoStaticStr, ::strum::Display,
        )]
        #[non_exhaustive]
        pub enum ChainId { #(#variants),* }

        impl ChainId {
            /// Every supported chain, in registry order.
            pub const ALL: &'static [Self] = &[#(Self::#names),*];
        }

        pub(crate) static CHAIN_IDS: [u64; #count] = [#(#chain_ids),*];
    };
    Ok((code, chain_variants))
}

fn generate_tokens(
    tokens: &BTreeMap<String, Vec<Value>>,
    chain_variants: &ChainVariants,
) -> Result<TokenStream> {
    let mut variants = Vec::new();
    let mut names = Vec::new();
    let mut records = Vec::new();
    let mut rows = Vec::new();
    let chain_count = chain_variants.values().flatten().count();
    let mut keys = HashMap::new();
    // (chain ID, normalized address), to reject addresses used by two tokens.
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
        // (ChainId ordinal, per-chain fields); the TokenId variant is only
        // made once the token is known to have a supported deployment.
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
            let Some((chain_variant, kind)) = chain_variant else {
                continue;
            };
            let address_code = if *kind == "Evm" {
                let valid_hex = address.strip_prefix("0x").is_some_and(|hex| {
                    hex.len() == 40 && hex.bytes().all(|b| b.is_ascii_hexdigit())
                });
                if !valid_hex {
                    return Err(format!("{at}: invalid EVM address {address:?}").into());
                }
                quote!(crate::TokenAddress::Evm(
                    ::alloy_primitives::address!(#address)
                ))
            } else {
                let decoder = bs58::decode(&address);
                let decoded = if *kind == "Tron" {
                    decoder.with_check(Some(0x41)).into_vec()
                } else {
                    decoder.into_vec()
                };
                if !decoded.is_ok_and(|bytes| bytes.len() == if *kind == "Tron" { 21 } else { 32 })
                {
                    return Err(format!("{at}: invalid {kind} address {address:?}").into());
                }
                let kind = format_ident!("{kind}");
                quote!(crate::TokenAddress::#kind(#address))
            };
            let address_key = if *kind == "Evm" {
                address.to_ascii_lowercase()
            } else {
                address.clone()
            };
            if !addresses.insert((chain_id, address_key)) {
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
                    address: #address_code,
                    decimals: #decimals,
                    native: #native,
                    name: #name,
                    symbol: #symbol,
                },
            ));
        }

        // Only deployed on unsupported chains.
        if deployments.is_empty() {
            continue;
        }

        let variant = variant("TokenId", id, &mut keys)?;
        let doc = format!(" `{id}`: {name}.");
        variants.push(quote!(#[doc = #doc] #[strum(serialize = #id)] #variant));
        names.push(variant.clone());

        let icon_url = format!("{TOKENLIST_BASE_URL}data/tokens/{id}/logo.{logo}");
        let mut row = vec![0_u16; chain_count];
        for (chain_variant, fields) in deployments {
            row[*chain_variant] = record_slot(records.len()).map_err(|e| format!("{id}: {e}"))?;
            records.push(quote! {
                TokenData {
                    id: TokenId::#variant,
                    #fields
                    coingecko_id: #coingecko_id,
                    icon_url: #icon_url,
                }
            });
        }
        rows.push(quote!([#(#row),*]));
    }

    let token_count = names.len();
    let record_count = records.len();
    Ok(quote! {
        /// A token with at least one supported deployment, named after its
        /// `data/tokens/` directory: `FromStr`, `Display` and
        /// [`TokenId::as_str`] use that exact ID string (`"BTC+"` is
        /// `TokenId::BtcPlus`).
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
            ::strum::EnumString, ::strum::IntoStaticStr, ::strum::Display,
        )]
        #[non_exhaustive]
        pub enum TokenId { #(#variants),* }

        impl TokenId {
            /// Every token with a supported deployment, in registry order.
            pub const ALL: &'static [Self] = &[#(Self::#names),*];
        }

        pub(crate) static RECORDS: [TokenData; #record_count] = [#(#records),*];

        // Zero means absent; other slots hold the record index plus one.
        pub(crate) static INDEX: [[u16; #chain_count]; #token_count] = [#(#rows),*];
    })
}

fn record_slot(index: usize) -> Result<u16> {
    index
        .checked_add(1)
        .and_then(|slot| u16::try_from(slot).ok())
        .ok_or_else(|| "more than 65535 deployments; widen the registry index".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn generate(chains: Value, tokens: Value) -> Result<String> {
        let chains = serde_json::from_value(chains)?;
        let tokens = serde_json::from_value(tokens)?;
        let (chain_code, ordinals) = generate_chains(&chains)?;
        let token_code = generate_tokens(&tokens, &ordinals)?;
        Ok(prettyplease::unparse(&syn::parse2(
            quote!(#chain_code #token_code),
        )?))
    }

    fn chains() -> Value {
        json!({
            "ethereum": {"chainId": 1, "evm": true},
            "solana": {"chainId": 1584368940, "evm": false},
            "tron": {"chainId": 728126428, "evm": false}
        })
    }

    fn entry() -> Value {
        json!([
            ["USD Coin", "USDC", "usd-coin", "svg"],
            [1, "0x0000000000000000000000000000000000000001", 6, false]
        ])
    }

    #[test]
    fn rejects_duplicate_json_keys_and_trailing_data() {
        for text in [r#"{"USDC": [], "USDC": []}"#, r#"{"USDC": []} {}"#] {
            assert!(parse_map::<Vec<Value>>(text).is_err());
        }
        assert!(
            parse_map::<SupportedChain>(
                r#"{"ethereum":{"chainId":1,"evm":true},"ethereum":{"chainId":2,"evm":true}}"#
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_duplicate_chain_ids_and_variant_collisions() {
        for (chains, message) in [
            (
                json!({
                    "ethereum": {"chainId": 1, "evm": true},
                    "solana": {"chainId": 1, "evm": false}
                }),
                "chain ID 1 is listed twice",
            ),
            (
                json!({
                    "foo-bar": {"chainId": 1, "evm": true},
                    "foo_bar": {"chainId": 2, "evm": true}
                }),
                "both map to ChainId::FooBar",
            ),
        ] {
            let error = generate(chains, json!({})).unwrap_err().to_string();
            assert!(error.contains(message), "{error}");
        }
        let mut other = entry();
        other[1][1] = json!("0x0000000000000000000000000000000000000002");
        let error = generate(chains(), json!({"BTC.b": entry(), "BTC_b": other}))
            .unwrap_err()
            .to_string();
        assert!(error.contains("both map to TokenId::BtcB"), "{error}");
    }

    #[test]
    fn rejects_invalid_enum_identifiers() {
        let error = generate(chains(), json!({"123": entry()}))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("123: TokenId::123 is not a valid identifier"),
            "{error}"
        );
    }

    #[test]
    fn rejects_duplicate_deployments_and_addresses() {
        let mut duplicate_chain = entry();
        let duplicate = duplicate_chain[1].clone();
        duplicate_chain.as_array_mut().unwrap().push(duplicate);
        let error = generate(chains(), json!({"USDC": duplicate_chain}))
            .unwrap_err()
            .to_string();
        assert!(error.contains("USDC[2]: duplicate chain 1"), "{error}");

        let mut first = entry();
        first[1][1] = json!("0xabcdef0000000000000000000000000000000001");
        let mut second = entry();
        second[1][1] = json!("0xABCDEF0000000000000000000000000000000001");
        let error = generate(chains(), json!({"FIRST": first, "SECOND": second}))
            .unwrap_err()
            .to_string();
        assert!(error.contains("SECOND[1]: another token uses"), "{error}");
    }

    #[test]
    fn rejects_unknown_chains_malformed_addresses_and_tuples() {
        for (slot, value, message) in [
            (0, json!(999), "chain 999 is not in supported-chains.json"),
            (1, json!("0x123"), "invalid EVM address"),
            (
                1,
                json!("0x000000000000000000000000000000000000000g"),
                "invalid EVM address",
            ),
            (2, json!(256), "invalid value"),
            (3, json!("false"), "invalid type"),
        ] {
            let mut malformed = entry();
            malformed[1][slot] = value;
            let error = generate(chains(), json!({"USDC": malformed}))
                .unwrap_err()
                .to_string();
            assert!(error.contains("USDC[1]"), "{error}");
            assert!(error.contains(message), "{error}");
        }
        let mut malformed = entry();
        malformed[1].as_array_mut().unwrap().push(Value::Null);
        malformed[1].as_array_mut().unwrap().push(Value::Null);
        malformed[1].as_array_mut().unwrap().push(json!("extra"));
        assert!(generate(chains(), json!({"USDC": malformed})).is_err());
    }

    #[test]
    fn rejects_missing_shared_metadata() {
        assert!(generate(chains(), json!({"USDC": []})).is_err());
        for (slot, value, message) in [
            (2, Value::Null, "missing coingeckoId"),
            (3, json!("png"), "unknown logo format"),
        ] {
            let mut malformed = entry();
            malformed[0][slot] = value;
            let error = generate(chains(), json!({"USDC": malformed}))
                .unwrap_err()
                .to_string();
            assert!(error.contains(message), "{error}");
        }
    }

    #[test]
    fn accepts_optional_overrides_and_non_evm_deployments() {
        let mut token = entry();
        token[1]
            .as_array_mut()
            .unwrap()
            .extend([Value::Null, json!("USDC.e")]);
        token.as_array_mut().unwrap().push(json!([
            1584368940,
            "So11111111111111111111111111111111111111112",
            6,
            false
        ]));
        assert!(generate(chains(), json!({"USDC": token})).is_ok());
        assert!(generate(chains(), json!({
            "SOL": [["Solana", "SOL", "solana", "svg"], [1584368940, "So11111111111111111111111111111111111111112", 9, true]]
        })).is_ok());
    }

    #[test]
    fn rejects_invalid_non_evm_addresses() {
        for (chain_id, address) in [
            (1584368940, "0OIl"),
            (1584368940, "1111"),
            (728126428, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6u"),
        ] {
            let mut token = entry();
            token[1][0] = json!(chain_id);
            token[1][1] = json!(address);
            let error = generate(chains(), json!({"USDT": token}))
                .unwrap_err()
                .to_string();
            assert!(error.contains("USDT[1]: invalid"), "{error}");
        }
    }

    #[test]
    fn record_slots_reserve_zero_and_check_capacity() {
        assert_eq!(record_slot(0).unwrap(), 1);
        assert_eq!(record_slot(65534).unwrap(), u16::MAX);
        assert!(record_slot(65535).is_err());
        assert!(record_slot(usize::MAX).is_err());
    }
}
