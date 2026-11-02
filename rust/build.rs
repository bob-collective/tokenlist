//! Generates `$OUT_DIR/generated.rs` from two files the npm build writes:
//! `../supported-chains.json` (`ChainId`) and `../compressedlist.json`
//! (`TokenId`, static `TokenData` records and metadata). Malformed or
//! inconsistent data fails the build, naming the entry.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env,
    error::Error,
    fs,
    path::PathBuf,
};

use proc_macro2::TokenStream;
use quote::quote;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use serde_with::{As, MapPreventDuplicates, Same};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SupportedChain {
    chain_id: u64,
    evm: bool,
}

/// `[name, symbol, coingeckoId, logo]`, the head of each `compressedlist.json` entry.
#[derive(Deserialize)]
struct Shared(String, String, Option<String>, String);

/// `[chainId, address, decimals, native, name?, symbol?]`; `name` is `null`
/// when only the symbol is overridden.
#[derive(Deserialize)]
struct ChainEntry(
    u64,
    String,
    u8,
    bool,
    #[serde(default)] Option<String>,
    #[serde(default)] Option<String>,
);

/// Address validation and code generation depend on the chain's address family.
#[derive(Debug, Clone, Copy)]
enum AddressKind {
    Evm,
    Bitcoin,
    Solana,
    Tron,
}

impl AddressKind {
    fn for_chain(key: &str, evm: bool) -> Option<Self> {
        match (evm, key) {
            (true, _) => Some(Self::Evm),
            (_, "bitcoin") => Some(Self::Bitcoin),
            (_, "solana") => Some(Self::Solana),
            (_, "tron") => Some(Self::Tron),
            _ => None,
        }
    }

    fn is_hex(self) -> bool {
        matches!(self, Self::Evm | Self::Bitcoin)
    }

    fn validate(self, address: &str) -> bool {
        match self {
            Self::Evm | Self::Bitcoin => address.strip_prefix("0x").is_some_and(|hex| {
                hex.len() == 40 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
            }),
            Self::Solana => bs58::decode(address)
                .into_vec()
                .is_ok_and(|bytes| bytes.len() == 32),
            Self::Tron => bs58::decode(address)
                .with_check(Some(0x41))
                .into_vec()
                .is_ok_and(|bytes| bytes.len() == 21),
        }
    }

    fn generate_address(self, address: &str) -> TokenStream {
        match self {
            Self::Evm => quote!(TokenAddress::Evm(::alloy_primitives::address!(#address))),
            Self::Bitcoin => quote!(TokenAddress::Bitcoin(
                ::alloy_primitives::address!(#address)
            )),
            Self::Solana => quote!(TokenAddress::Solana(#address)),
            Self::Tron => quote!(TokenAddress::Tron(#address)),
        }
    }
}

struct Chain {
    index: usize,
    variant: syn::Ident,
    address_kind: AddressKind,
}

/// Includes unsupported chains so unknown IDs can be distinguished from them.
type Chains = HashMap<u64, Option<Chain>>;

struct Deployment {
    chain_index: usize,
    fields: TokenStream,
}

fn main() -> Result<()> {
    let (chains, chain_lookup) = read_map("supported-chains.json")
        .and_then(|chains| generate_chains(&chains))
        .map_err(|error| format!("supported-chains.json: {error}"))?;
    let tokens = read_map("compressedlist.json")
        .and_then(|tokens| generate_tokens(&tokens, &chain_lookup))
        .map_err(|error| format!("compressedlist.json: {error}"))?;
    let out =
        PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?).join("generated.rs");
    fs::write(out, quote!(#chains #tokens).to_string())?;
    Ok(())
}

/// A JSON object at the repo root.
fn read_map<T: DeserializeOwned>(file: &str) -> Result<BTreeMap<String, T>> {
    let path = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?)
        .join("..")
        .join(file);
    println!("cargo::rerun-if-changed={}", path.display());
    parse_map(&fs::read_to_string(path)?)
}

/// Rejects duplicate keys and trailing data.
fn parse_map<T: DeserializeOwned>(text: &str) -> Result<BTreeMap<String, T>> {
    let mut de = serde_json::Deserializer::from_str(text);
    let map = As::<MapPreventDuplicates<Same, Same>>::deserialize(&mut de)?;
    de.end()?;
    Ok(map)
}

/// The variant for a token ID or chain key, named as bob-gateway's
/// `TokenSymbol`: `+` becomes `_plus_` and `.` `_dot_`, then each run of
/// letters and digits is capitalized with the rest lowercased (`XAUt` is
/// `Xaut`, `cbBTC` `Cbbtc`, `BTC.b` `BtcDotB`, `op-mainnet` `OpMainnet`).
/// Rejects invalid identifiers and keys sharing a variant.
fn variant(enum_name: &str, key: &str, keys: &mut HashMap<String, String>) -> Result<syn::Ident> {
    let mut name = String::new();
    let key_words = key.replace('+', "_plus_").replace('.', "_dot_");
    for word in key_words.split(|c: char| !c.is_alphanumeric()) {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            name.extend(first.to_uppercase());
            name.extend(chars.flat_map(char::to_lowercase));
        }
    }
    let ident = syn::parse_str(&name)
        .map_err(|e| format!("{key}: {enum_name}::{name} is not a valid identifier: {e}"))?;
    if let Some(other) = keys.insert(name.clone(), key.to_owned()) {
        return Err(format!("{key} and {other} both map to {enum_name}::{name}").into());
    }
    Ok(ident)
}

/// The `ChainId` enum, metadata and build-time lookup by numeric ID.
fn generate_chains(chains: &BTreeMap<String, SupportedChain>) -> Result<(TokenStream, Chains)> {
    let mut variants = Vec::new();
    let mut names = Vec::new();
    let mut metadata = Vec::new();
    let mut lookup = HashMap::new();
    let mut keys = HashMap::new();

    for (key, &SupportedChain { chain_id, evm }) in chains {
        let chain = if let Some(address_kind) = AddressKind::for_chain(key, evm) {
            let variant = variant("ChainId", key, &mut keys)?;
            let doc = format!(" `{key}` (chain ID {chain_id}).");
            let index = names.len();
            variants.push(quote!(#[doc = #doc] #variant));
            names.push(variant.clone());
            metadata.push(quote!(ChainMetadata { key: #key, chain_id: #chain_id }));
            Some(Chain {
                index,
                variant,
                address_kind,
            })
        } else {
            None
        };
        if lookup.insert(chain_id, chain).is_some() {
            return Err(format!("{key}: chain ID {chain_id} is listed twice").into());
        }
    }

    let code = quote! {
        /// A supported chain, named after its `data.json` key: `FromStr`, `Display`
        /// and [`ChainId::as_str`] use that key (`"op-mainnet"` is
        /// `ChainId::OpMainnet`).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[non_exhaustive]
        pub enum ChainId { #(#variants),* }

        impl ChainId {
            /// Every supported chain, sorted by key.
            pub const ALL: &'static [Self] = &[#(Self::#names),*];
        }

        static CHAINS: &[ChainMetadata] = &[#(#metadata),*];
    };
    Ok((code, lookup))
}

/// The `TokenId` enum, records and metadata with each token's deployment range.
fn generate_tokens(tokens: &BTreeMap<String, Vec<Value>>, chains: &Chains) -> Result<TokenStream> {
    let mut variants = Vec::new();
    let mut names = Vec::new();
    let mut metadata = Vec::new();
    let mut records = Vec::new();
    let mut keys = HashMap::new();
    // (chain ID, normalized address), to reject addresses shared by two tokens.
    let mut addresses = HashSet::new();

    for (id, entry) in tokens {
        let (shared, entries) = entry
            .split_first()
            .ok_or_else(|| format!("{id}: empty entry"))?;
        let Shared(name, symbol, coingecko_id, logo) =
            Shared::deserialize(shared).map_err(|error| format!("{id}[0]: {error}"))?;
        let coingecko_id = coingecko_id.ok_or_else(|| format!("{id}: missing coingeckoId"))?;
        if !matches!(logo.as_str(), "svg" | "webp") {
            return Err(format!("{id}[0]: unknown logo format {logo:?}").into());
        }

        let mut seen_chains = HashSet::new();
        let mut deployments = Vec::new();

        for (index, entry) in entries.iter().enumerate() {
            let at = format!("{id}[{}]", index + 1);
            let ChainEntry(chain_id, address, decimals, native, name_override, symbol_override) =
                ChainEntry::deserialize(entry).map_err(|error| format!("{at}: {error}"))?;
            if !seen_chains.insert(chain_id) {
                return Err(format!("{at}: duplicate chain {chain_id}").into());
            }
            let Some(chain) = chains.get(&chain_id).ok_or_else(|| {
                format!(
                    "{at}: chain {chain_id} is not in supported-chains.json; add it to typescript/config.ts and run `pnpm build`"
                )
            })? else {
                continue;
            };

            let kind = chain.address_kind;
            if !kind.validate(&address) {
                return Err(format!("{at}: invalid {kind:?} address {address:?}").into());
            }
            let address_key = if kind.is_hex() {
                address.to_ascii_lowercase()
            } else {
                address.clone()
            };
            if !addresses.insert((chain_id, address_key)) {
                return Err(
                    format!("{at}: another token uses {address} on chain {chain_id}").into(),
                );
            }

            let chain_variant = &chain.variant;
            let address = kind.generate_address(&address);
            let name = name_override.as_deref().unwrap_or(&name);
            let symbol = symbol_override.as_deref().unwrap_or(&symbol);
            deployments.push(Deployment {
                chain_index: chain.index,
                fields: quote! {
                    chain: ChainId::#chain_variant,
                    address: #address,
                    decimals: #decimals,
                    native: #native,
                    coingecko_id: #coingecko_id,
                    name: #name,
                    symbol: #symbol,
                },
            });
        }

        // Tokens deployed only on unsupported chains have no TokenId.
        if deployments.is_empty() {
            continue;
        }

        let variant = variant("TokenId", id, &mut keys)?;
        let doc = format!(" `{id}`: {name}.");
        variants.push(quote!(#[doc = #doc] #variant));
        names.push(variant.clone());

        deployments.sort_by_key(|deployment| deployment.chain_index);
        let start = records.len();
        for deployment in deployments {
            let fields = deployment.fields;
            records.push(quote!(TokenData { id: TokenId::#variant, #fields }));
        }
        let end = records.len();
        metadata.push(quote! {
            TokenMetadata { id: #id, logo_extension: #logo, deployments: #start..#end }
        });
    }

    let record_count = records.len();
    Ok(quote! {
        /// A token with at least one supported deployment, named after its
        /// `data/tokens/` directory: `FromStr`, `Display` and
        /// [`TokenId::as_str`] use that exact ID string (`"BTC+"` is
        /// `TokenId::BtcPlus`).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[non_exhaustive]
        pub enum TokenId { #(#variants),* }

        impl TokenId {
            /// Every token with a supported deployment, sorted by ID.
            pub const ALL: &'static [Self] = &[#(Self::#names),*];
        }

        static RECORDS: [TokenData; #record_count] = [#(#records),*];
        static TOKENS: &[TokenMetadata] = &[#(#metadata),*];
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Generates code from the inputs and checks that it parses.
    fn generate(chains: Value, tokens: Value) -> Result<()> {
        let (chains, chain_lookup) = generate_chains(&serde_json::from_value(chains)?)?;
        let tokens = generate_tokens(&serde_json::from_value(tokens)?, &chain_lookup)?;
        syn::parse2::<syn::File>(quote!(#chains #tokens))?;
        Ok(())
    }

    fn assert_fails(chains: Value, tokens: Value, message: &str) {
        let error = generate(chains, tokens).unwrap_err().to_string();
        assert!(error.contains(message), "{error}");
    }

    fn chains() -> Value {
        json!({
            "ethereum": {"chainId": 1, "evm": true},
            "solana": {"chainId": 1584368940, "evm": false},
            "tron": {"chainId": 728126428, "evm": false}
        })
    }

    #[test]
    fn rejects_invalid_data() {
        for text in [r#"{"USDC": [], "USDC": []}"#, r#"{"USDC": []} {}"#] {
            assert!(parse_map::<Vec<Value>>(text).is_err());
        }

        #[rustfmt::skip]
        let chain_cases = [
            (json!({"ethereum": {"chainId": 1, "evm": true}, "solana": {"chainId": 1, "evm": false}}), "chain ID 1 is listed twice"),
            (json!({"foo-bar": {"chainId": 1, "evm": true}, "foo_bar": {"chainId": 2, "evm": true}}), "both map to ChainId::FooBar"),
        ];
        for (chains, message) in chain_cases {
            assert_fails(chains, json!({}), message);
        }

        let shared = json!(["USD Coin", "USDC", "usd-coin", "svg"]);
        let evm = |address: &str| json!([1, address, 6, false]);
        let ok = evm("0x0000000000000000000000000000000000000001");
        #[rustfmt::skip]
        let token_cases = [
            (json!({"USDC": []}), "USDC: empty entry"),
            (json!({"USDC": [["USD Coin", "USDC", null, "svg"], ok]}), "USDC: missing coingeckoId"),
            (json!({"USDC": [["USD Coin", "USDC", "usd-coin", "png"], ok]}), "USDC[0]: unknown logo format"),
            (json!({"USDC": [shared, ok, ok]}), "USDC[2]: duplicate chain 1"),
            (json!({"USDC": [shared, [999, "0x0000000000000000000000000000000000000001", 6, false]]}), "USDC[1]: chain 999 is not in supported-chains.json"),
            (json!({"USDC": [shared, evm("0x123")]}), "USDC[1]: invalid Evm address"),
            (json!({"USDC": [shared, evm("0x000000000000000000000000000000000000000g")]}), "USDC[1]: invalid Evm address"),
            (json!({"USDC": [shared, [1, "0x0000000000000000000000000000000000000001", 256, false]]}), "USDC[1]: invalid value"),
            (json!({"USDC": [shared, [1, "0x0000000000000000000000000000000000000001", 6, "false"]]}), "USDC[1]: invalid type"),
            (json!({"USDC": [shared, [1, "0x0000000000000000000000000000000000000001", 6, false, null, null, "extra"]]}), "USDC[1]: invalid length"),
            (json!({"USDT": [shared, [1584368940, "0OIl", 6, false]]}), "USDT[1]: invalid Solana address"),
            (json!({"USDT": [shared, [1584368940, "1111", 6, false]]}), "USDT[1]: invalid Solana address"),
            (json!({"USDT": [shared, [728126428, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6u", 6, false]]}), "USDT[1]: invalid Tron address"),
            (json!({"A": [shared, evm("0xabcdef0000000000000000000000000000000001")], "B": [shared, evm("0xABCDEF0000000000000000000000000000000001")]}), "B[1]: another token uses"),
            (json!({"123": [shared, ok]}), "123: TokenId::123 is not a valid identifier"),
            (json!({"XAUt": [shared, ok], "XAUT": [shared, evm("0x0000000000000000000000000000000000000002")]}), "both map to TokenId::Xaut"),
        ];
        for (tokens, message) in token_cases {
            assert_fails(chains(), tokens, message);
        }
    }

    #[test]
    fn accepts_overrides_and_non_evm_deployments() {
        let tokens = json!({"USDC": [
            ["USD Coin", "USDC", "usd-coin", "svg"],
            [1, "0x0000000000000000000000000000000000000001", 6, false, null, "USDC.e"],
            [1584368940, "So11111111111111111111111111111111111111112", 6, false],
        ]});
        generate(chains(), tokens).unwrap();
    }

    #[test]
    fn capitalizes_each_word_and_lowercases_the_rest() {
        let mut keys = HashMap::new();
        for (key, expected) in [
            ("XAUt", "Xaut"),
            ("XAUt0", "Xaut0"),
            ("cbBTC", "Cbbtc"),
            ("tBTC", "Tbtc"),
            ("BTC.b", "BtcDotB"),
            ("SolvBTC.JUP", "SolvbtcDotJup"),
            ("BTC+", "BtcPlus"),
            ("op-mainnet", "OpMainnet"),
        ] {
            assert_eq!(variant("TokenId", key, &mut keys).unwrap(), expected);
        }
    }

    #[test]
    fn accepts_more_than_64_chains_and_large_strings() {
        let chains: serde_json::Map<_, _> = (0..65)
            .map(|id| (format!("c{id}"), json!({"chainId": id, "evm": true})))
            .collect();
        let deployments: Vec<_> = (0..65)
            .map(|id| json!([id, "0x0000000000000000000000000000000000000001", 6, false]))
            .collect();
        let mut entry = vec![json!(["x".repeat(65536), "USDC", "usd-coin", "svg"])];
        entry.extend(deployments);
        generate(chains.into(), json!({"USDC": entry})).unwrap();
    }
}
