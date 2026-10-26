//! Generates `$OUT_DIR/generated.rs` from two files the npm build writes:
//! `../supported-chains.json` (`ChainId`) and `../compressedlist.json`
//! (`TokenId`, a flat `TokenData` array, a per-token index and one string
//! table). Malformed or inconsistent data fails the build, naming the entry.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::{env, error::Error, fs, path::PathBuf};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use serde_with::{As, MapPreventDuplicates, Same};

type Result<T, E = Box<dyn Error>> = std::result::Result<T, E>;

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

/// Numeric chain ID → (`ChainId` ordinal and variant, address kind), `None`
/// if unsupported.
type Ordinals = HashMap<u64, Option<(usize, syn::Ident, &'static str)>>;

/// The generated `TEXT` table: every string concatenated, split at `TEXT_ENDS`.
#[derive(Default)]
struct Texts {
    text: String,
    ends: Vec<u16>,
    index: HashMap<String, u16>,
}

impl Texts {
    /// Appends `s` even if present, so keys stay contiguous.
    fn push(&mut self, s: &str) -> Result<u16> {
        self.text.push_str(s);
        let (Ok(index), Ok(end)) = (self.ends.len().try_into(), self.text.len().try_into()) else {
            return Err("more than 65535 bytes of strings; widen TEXT_ENDS".into());
        };
        self.ends.push(end);
        self.index.entry(s.to_owned()).or_insert(index);
        Ok(index)
    }

    /// `s`'s index, appending it if new.
    fn intern(&mut self, s: &str) -> Result<u16> {
        match self.index.get(s) {
            Some(&index) => Ok(index),
            None => self.push(s),
        }
    }

    fn generate(&self) -> TokenStream {
        let (text, ends, count) = (&self.text, &self.ends, self.ends.len());
        quote! {
            static TEXT: &str = #text;
            static TEXT_ENDS: [u16; #count] = [#(#ends),*];
        }
    }
}

fn main() -> Result<()> {
    let mut texts = Texts::default();
    let (chains, ordinals) = read_map("supported-chains.json")
        .and_then(|chains| generate_chains(&chains, &mut texts))
        .map_err(|e| format!("supported-chains.json: {e}"))?;
    let tokens = read_map("compressedlist.json")
        .and_then(|tokens| generate_tokens(&tokens, &ordinals, &mut texts))
        .map_err(|e| format!("compressedlist.json: {e}"))?;
    let text = texts.generate();
    let out = PathBuf::from(env::var("OUT_DIR")?).join("generated.rs");
    Ok(fs::write(out, quote!(#chains #tokens #text).to_string())?)
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

/// The `ChainId` enum and each chain ID's ordinal. Chain keys open `TEXT`,
/// at their ordinals.
fn generate_chains(
    chains: &BTreeMap<String, SupportedChain>,
    texts: &mut Texts,
) -> Result<(TokenStream, Ordinals)> {
    let (mut variants, mut names, mut ids) = (vec![], vec![], vec![]);
    let (mut ordinals, mut keys) = (HashMap::new(), HashMap::new());

    for (key, &SupportedChain { chain_id: id, evm }) in chains {
        let kind = match (evm, key.as_str()) {
            (true, _) => Some("Evm"),
            (_, "bitcoin") => Some("Bitcoin"),
            (_, "solana") => Some("Solana"),
            (_, "tron") => Some("Tron"),
            _ => None,
        };
        let entry = match kind {
            Some(kind) => {
                let variant = variant("ChainId", key, &mut keys)?;
                let doc = format!(" `{key}` (chain ID {id}).");
                variants.push(quote!(#[doc = #doc] #variant));
                texts.push(key)?;
                ids.push(id);
                names.push(variant.clone());
                Some((names.len() - 1, variant, kind))
            }
            None => None,
        };
        if ordinals.insert(id, entry).is_some() {
            return Err(format!("{key}: chain ID {id} is listed twice").into());
        }
    }

    let count = names.len();
    if count > 64 {
        return Err("more than 64 supported chains; widen DEPLOYED".into());
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

        static CHAIN_IDS: [u64; #count] = [#(#ids),*];
    };
    Ok((code, ordinals))
}

/// The `TokenId` enum, the `TokenData` records and the per-token index.
/// Token IDs follow the chain keys in `TEXT`, then names and symbols.
fn generate_tokens(
    tokens: &BTreeMap<String, Vec<Value>>,
    ordinals: &Ordinals,
    texts: &mut Texts,
) -> Result<TokenStream> {
    let (mut variants, mut names, mut kept) = (vec![], vec![], vec![]);
    let (mut deployed, mut first, mut webp, mut record_count) = (vec![], vec![], vec![], 0);
    let mut keys = HashMap::new();
    // (chain ID, normalized address), to reject addresses shared by two tokens.
    let mut addresses = HashSet::new();

    for (id, entry) in tokens {
        let (shared, chains) = entry.split_first().ok_or(format!("{id}: empty entry"))?;
        let Shared(name, symbol, coingecko_id, logo) =
            Shared::deserialize(shared).map_err(|e| format!("{id}[0]: {e}"))?;
        let coingecko_id = coingecko_id.ok_or(format!("{id}: missing coingeckoId"))?;
        if !matches!(logo.as_str(), "svg" | "webp") {
            return Err(format!("{id}[0]: unknown logo format {logo:?}").into());
        }

        let mut seen_chains = HashSet::new();
        // (ChainId ordinal, record fields, name, symbol); the TokenId variant
        // is only made once the token is known to have a supported deployment.
        let mut deployments = vec![];

        for (index, chain) in chains.iter().enumerate() {
            let at = format!("{id}[{}]", index + 1);
            let ChainEntry(chain_id, address, decimals, native, name_override, symbol_override) =
                ChainEntry::deserialize(chain).map_err(|e| format!("{at}: {e}"))?;
            if !seen_chains.insert(chain_id) {
                return Err(format!("{at}: duplicate chain {chain_id}").into());
            }
            let Some((ordinal, chain, kind)) = ordinals.get(&chain_id).ok_or_else(|| {
                format!(
                    "{at}: chain {chain_id} is not in supported-chains.json; add it to typescript/config.ts and run `pnpm build`"
                )
            })?
            else {
                continue;
            };

            let hex = matches!(*kind, "Evm" | "Bitcoin");
            let valid = if hex {
                address.strip_prefix("0x").is_some_and(|hex| {
                    hex.len() == 40 && hex.bytes().all(|b| b.is_ascii_hexdigit())
                })
            } else if *kind == "Tron" {
                let decoded = bs58::decode(&address).with_check(Some(0x41)).into_vec();
                decoded.is_ok_and(|bytes| bytes.len() == 21)
            } else {
                bs58::decode(&address)
                    .into_vec()
                    .is_ok_and(|bytes| bytes.len() == 32)
            };
            if !valid {
                return Err(format!("{at}: invalid {kind} address {address:?}").into());
            }
            let address_key = if hex {
                address.to_ascii_lowercase()
            } else {
                address.clone()
            };
            if !addresses.insert((chain_id, address_key)) {
                return Err(
                    format!("{at}: another token uses {address} on chain {chain_id}").into(),
                );
            }

            let kind = format_ident!("{kind}");
            let address = if hex {
                quote!(::alloy_primitives::address!(#address))
            } else {
                quote!(#address)
            };
            let fields = quote! {
                chain: ChainId::#chain,
                address: TokenAddress::#kind(#address),
                decimals: #decimals,
                native: #native,
                coingecko_id: #coingecko_id,
            };
            let name = name_override.unwrap_or_else(|| name.clone());
            let symbol = symbol_override.unwrap_or_else(|| symbol.clone());
            deployments.push((*ordinal, fields, name, symbol));
        }

        // Only deployed on unsupported chains.
        if deployments.is_empty() {
            continue;
        }

        let variant = variant("TokenId", id, &mut keys)?;
        let doc = format!(" `{id}`: {name}.");
        variants.push(quote!(#[doc = #doc] #variant));
        names.push(variant.clone());
        webp.push(logo == "webp");

        // In ChainId order, so the record for a chain is the token's first
        // plus its deployments on lower ordinals.
        deployments.sort_by_key(|deployment| deployment.0);
        first.push(first_record(record_count).map_err(|e| format!("{id}: {e}"))?);
        record_count += deployments.len();
        deployed.push(
            deployments
                .iter()
                .fold(0_u64, |mask, deployment| mask | 1 << deployment.0),
        );
        kept.push((id, variant, deployments));
    }

    // Token IDs first, contiguous, so `TokenId::as_str` indexes them directly.
    for (id, _, _) in &kept {
        texts.push(id)?;
    }
    let mut records = vec![];
    for (_, variant, deployments) in kept {
        for (_, fields, name, symbol) in deployments {
            let (name, symbol) = (texts.intern(&name)?, texts.intern(&symbol)?);
            records.push(quote! {
                TokenData { id: TokenId::#variant, #fields name: Text(#name), symbol: Text(#symbol) }
            });
        }
    }

    let token_count = names.len();
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

        /// Per token, a bit for each `ChainId` ordinal it is deployed on.
        static DEPLOYED: [u64; #token_count] = [#(#deployed),*];

        /// Per token, the `RECORDS` index of its first deployment.
        static FIRST: [u16; #token_count] = [#(#first),*];

        /// Per token, whether its logo is `logo.webp` rather than `logo.svg`.
        static LOGO_WEBP: [bool; #token_count] = [#(#webp),*];
    })
}

/// `FIRST` entry for a token whose records start at `index`.
fn first_record(index: usize) -> Result<u16> {
    u16::try_from(index).map_err(|_| "more than 65535 deployments; widen FIRST".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Generates code from the inputs and checks that it parses.
    fn generate(chains: Value, tokens: Value) -> Result<()> {
        let mut texts = Texts::default();
        let (chains, ordinals) = generate_chains(&serde_json::from_value(chains)?, &mut texts)?;
        let tokens = generate_tokens(&serde_json::from_value(tokens)?, &ordinals, &mut texts)?;
        let text = texts.generate();
        syn::parse2::<syn::File>(quote!(#chains #tokens #text))?;
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

        let many: serde_json::Map<_, _> = (0..65)
            .map(|id| (format!("c{id}"), json!({"chainId": id, "evm": true})))
            .collect();
        #[rustfmt::skip]
        let chain_cases = [
            (json!({"ethereum": {"chainId": 1, "evm": true}, "solana": {"chainId": 1, "evm": false}}), "chain ID 1 is listed twice"),
            (json!({"foo-bar": {"chainId": 1, "evm": true}, "foo_bar": {"chainId": 2, "evm": true}}), "both map to ChainId::FooBar"),
            (many.into(), "more than 64 supported chains"),
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
    fn first_record_checks_capacity() {
        assert_eq!(first_record(65535).unwrap(), u16::MAX);
        assert!(first_record(65536).is_err() && first_record(usize::MAX).is_err());
    }
}
