//! Generates the chain, token ID and token data tables included by
//! `src/lib.rs` (`$OUT_DIR/generated.rs`) from the repository's JSON files:
//!
//! - `supported-chains.json`: chain keys, IDs, names, EVM and testnet flags;
//! - `data/chains/chains.json`: chain logo file names;
//! - `data/tokens/<ID>/data.json` (+ `logo.svg` or `logo.webp`): token data.

use std::collections::HashSet;
use std::fmt::{self, Write as _};
use std::path::{Path, PathBuf};
use std::{env, fs};

use serde::Deserialize;
use serde::de::{DeserializeOwned, MapAccess, Visitor};
use serde_json::{Map, Value};

// Shared with the library, so addresses are checksummed exactly as
// `checksum_address` does.
#[allow(dead_code)]
#[path = "src/address.rs"]
mod address;

const SUPPORTED_CHAINS: &str = "supported-chains.json";
const CHAIN_LOGOS: &str = "data/chains/chains.json";
const TOKEN_DIR: &str = "data/tokens";

// Names that cannot be used as enum variants as is: strict and reserved Rust
// keywords (matched case-sensitively), `_`, which is not a valid identifier on
// its own, and the associated items `src/macros.rs` and the crate define on
// `Chain` and the token ID enums, which a variant of the same name would shadow.
const RESERVED_IDENTS: &[&str] = &[
    "ALL",
    "as_str",
    "data",
    "from_id",
    "id",
    "is_evm",
    "is_testnet",
    "logo_file",
    "logo_uri",
    "mirror_logo_uri",
    "token",
    "token_id",
];
const RUST_KEYWORDS: &[&str] = &[
    "_", "abstract", "as", "async", "await", "become", "box", "break", "const", "continue",
    "crate", "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if",
    "impl", "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "try", "type",
    "typeof", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

type Object = Map<String, Value>;

// A JSON object's entries in document order. `Map` sorts its keys unless
// serde_json's `preserve_order` feature is on, and enabling that here would
// change serde_json for every proc macro and build script of a consumer.
struct Entries(Vec<(String, Value)>);

impl<'de> Deserialize<'de> for Entries {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct EntriesVisitor;

        impl<'de> Visitor<'de> for EntriesVisitor {
            type Value = Entries;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a JSON object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Entries, A::Error> {
                let mut entries = Vec::new();

                while let Some(entry) = map.next_entry()? {
                    entries.push(entry);
                }

                Ok(Entries(entries))
            }
        }

        deserializer.deserialize_map(EntriesVisitor)
    }
}

struct Chain {
    key: String,
    ident: String,
    id: u64,
    name: String,
    evm: bool,
    testnet: bool,
    logo: Option<String>,
}

struct Token {
    id: String,
    ident: String,
    data: Object,
    logo_ext: &'static str,
}

fn main() {
    for path in [SUPPORTED_CHAINS, CHAIN_LOGOS, TOKEN_DIR] {
        println!("cargo::rerun-if-changed={path}");
    }

    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let chains = read_chains(&root);
    let tokens = read_tokens(&root);

    assert_unique_idents("chain", chains.iter().map(|chain| &chain.ident));
    assert_unique_idents("token", tokens.iter().map(|token| &token.ident));

    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("generated.rs");

    fs::write(out, generate(&chains, &tokens)).unwrap();
}

fn read_json<T: DeserializeOwned>(path: &Path) -> T {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));

    serde_json::from_str(&source)
        .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()))
}

fn as_object<'a>(value: &'a Value, context: &str) -> &'a Object {
    value
        .as_object()
        .unwrap_or_else(|| panic!("{context}: expected an object"))
}

// An optional field; a value of the wrong type fails the build.
fn optional<'a, T>(
    object: &'a Object,
    key: &str,
    context: &str,
    convert: fn(&'a Value) -> Option<T>,
) -> Option<T> {
    object.get(key).map(|value| {
        convert(value).unwrap_or_else(|| panic!("{context}: invalid `{key}`: {value}"))
    })
}

fn required<'a, T>(
    object: &'a Object,
    key: &str,
    context: &str,
    convert: fn(&'a Value) -> Option<T>,
) -> T {
    optional(object, key, context, convert).unwrap_or_else(|| panic!("{context}: missing `{key}`"))
}

fn as_u8(value: &Value) -> Option<u8> {
    value.as_u64().and_then(|value| u8::try_from(value).ok())
}

fn read_chains(root: &Path) -> Vec<Chain> {
    let logos: Value = read_json(&root.join(CHAIN_LOGOS));
    let Entries(supported) = read_json(&root.join(SUPPORTED_CHAINS));

    supported
        .iter()
        .map(|(key, chain)| {
            let context = format!("{SUPPORTED_CHAINS}: {key}");
            let chain = as_object(chain, &context);

            Chain {
                key: key.clone(),
                ident: chain_ident(key),
                id: required(chain, "chainId", &context, Value::as_u64),
                name: required(chain, "name", &context, Value::as_str).to_owned(),
                evm: required(chain, "evm", &context, Value::as_bool),
                testnet: required(chain, "testnet", &context, Value::as_bool),
                logo: logos.get(key).and_then(Value::as_str).map(str::to_owned),
            }
        })
        .collect()
}

fn read_tokens(root: &Path) -> Vec<Token> {
    let token_dir = root.join(TOKEN_DIR);
    let mut ids: Vec<String> = fs::read_dir(&token_dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", token_dir.display()))
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().into_string().unwrap())
        .collect();

    // Case-insensitive, then by ID so IDs differing only in case keep a fixed order.
    ids.sort_by_key(|id| (id.to_lowercase(), id.clone()));

    ids.into_iter()
        .map(|id| {
            let dir = token_dir.join(&id);
            let data: Value = read_json(&dir.join("data.json"));
            // Exactly one logo: with both, the npm build picks whichever its
            // glob returns first.
            let logo_ext = match (
                dir.join("logo.webp").is_file(),
                dir.join("logo.svg").is_file(),
            ) {
                (true, false) => "Webp",
                (false, true) => "Svg",
                (true, true) => panic!("{TOKEN_DIR}/{id}: both logo.svg and logo.webp; keep one"),
                (false, false) => panic!("{TOKEN_DIR}/{id}: missing logo.svg or logo.webp"),
            };

            Token {
                ident: token_ident(&id),
                data: as_object(&data, &format!("{TOKEN_DIR}/{id}/data.json")).clone(),
                logo_ext,
                id,
            }
        })
        .collect()
}

// Chain keys are kebab-case `data.json` keys: `op-mainnet` → `OpMainnet`.
fn chain_ident(key: &str) -> String {
    let ident = key
        .split('-')
        .map(|part| {
            let mut chars = part.chars();

            chars
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect();

    variant_ident(ident, &format!("chain key {key:?}"), "chain_ident")
}

// Token IDs keep their original casing as enum variants (`TokenId::cbBTC`);
// characters that are not valid in identifiers are spelled out.
fn token_ident(id: &str) -> String {
    let ident = id.replace('.', "_").replace('+', "_PLUS");

    variant_ident(ident, &format!("token ID {id:?}"), "token_ident")
}

// Turns a candidate into a valid enum variant name, or fails the build naming
// the `source` entry and the `mapper` function to extend.
fn variant_ident(mut ident: String, source: &str, mapper: &str) -> String {
    assert!(
        !ident.is_empty()
            && ident
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_'),
        "cannot map {source} to a Rust identifier; extend `{mapper}` in build.rs",
    );
    if ident.starts_with(|c: char| c.is_ascii_digit()) {
        ident.insert(0, '_');
    }
    if RUST_KEYWORDS.contains(&ident.as_str()) || RESERVED_IDENTS.contains(&ident.as_str()) {
        ident.push('_');
    }

    ident
}

fn assert_unique_idents<'a>(kind: &str, idents: impl Iterator<Item = &'a String>) {
    let mut seen = HashSet::new();

    for ident in idents {
        assert!(
            seen.insert(ident),
            "duplicate Rust {kind} identifier: {ident}"
        );
    }
}

// A `#[doc]` attribute: single line, with Markdown that rustdoc would read as
// links or HTML escaped.
fn doc(text: &str) -> String {
    let mut escaped = String::new();

    for word in text.split_whitespace() {
        if !escaped.is_empty() {
            escaped.push(' ');
        }
        for c in word.chars() {
            if matches!(c, '\\' | '`' | '*' | '[' | ']' | '<' | '>') {
                escaped.push('\\');
            }
            escaped.push(c);
        }
    }

    format!("#[doc = {:?}]", format!(" {escaped}"))
}

// Position of `key` in `Chain::ALL`.
fn chain_index(chains: &[Chain], key: &str, context: &str) -> usize {
    chains
        .iter()
        .position(|chain| chain.key == key)
        .unwrap_or_else(|| panic!("{context}: unknown chain {key:?}, not in {SUPPORTED_CHAINS}"))
}

fn find_chain<'a>(chains: &'a [Chain], key: &str, context: &str) -> &'a Chain {
    &chains[chain_index(chains, key, context)]
}

fn generate(chains: &[Chain], tokens: &[Token]) -> String {
    let mut out = String::new();

    out.push_str(
        "// Generated by build.rs from supported-chains.json, data/chains/chains.json\n\
         // and data/tokens/*/data.json.\n\n\
         use crate::{ChainTokenData, LogoExt, Overrides, TokenData};\n\n\
         define_chains! {\n",
    );
    for chain in chains {
        writeln!(
            out,
            "    {}\n    {} {{ name: {:?}, id: {}, evm: {}, testnet: {}, logo: {:?} }},",
            doc(&chain.name),
            chain.ident,
            chain.key,
            chain.id,
            chain.evm,
            chain.testnet,
            chain.logo.as_deref(),
        )
        .unwrap();
    }

    out.push_str("}\n\ndefine_token_ids! {\n");
    let names: Vec<String> = tokens
        .iter()
        .map(|token| {
            let context = format!("{TOKEN_DIR}/{}/data.json", token.id);

            doc(required(&token.data, "name", &context, Value::as_str))
        })
        .collect();
    for (token, name) in tokens.iter().zip(&names) {
        writeln!(out, "    {name}\n    {} => {:?},", token.ident, token.id).unwrap();
    }
    out.push_str("}\n");

    let is_native = |token: &Token| token.data.get("native") == Some(&Value::Bool(true));
    let is_non_evm = |token: &Token| {
        token
            .data
            .get("tokens")
            .and_then(Value::as_object)
            .is_some_and(|entries| {
                entries
                    .keys()
                    .any(|key| chains.iter().any(|chain| chain.key == *key && !chain.evm))
            })
    };
    let native: Vec<bool> = tokens.iter().map(is_native).collect();
    let non_evm: Vec<bool> = tokens.iter().map(is_non_evm).collect();
    let non_evm_native: Vec<bool> = native.iter().zip(&non_evm).map(|(a, b)| *a && *b).collect();
    let subsets = [
        (
            "NativeTokenId",
            "Token IDs marked `\"native\": true` in their `data.json`.",
            &native,
        ),
        (
            "NonEvmTokenId",
            "Token IDs with at least one entry on a non-EVM chain (Solana, Bitcoin networks, Tron).",
            &non_evm,
        ),
        (
            "NonEvmNativeTokenId",
            "Native token IDs with at least one entry on a non-EVM chain.",
            &non_evm_native,
        ),
    ];
    for (name, description, members) in subsets {
        write!(
            out,
            "\ndefine_token_id_subset! {{\n    #[doc = {description:?}]\n    {name} {{\n"
        )
        .unwrap();
        for ((token, doc), _) in tokens
            .iter()
            .zip(&names)
            .zip(members)
            .filter(|(_, member)| **member)
        {
            writeln!(out, "        {doc}\n        {},", token.ident).unwrap();
        }
        out.push_str("    }\n}\n");
    }

    writeln!(
        out,
        "\n/// Token data indexed by `TokenId as usize`.\npub(crate) static TOKENS: [TokenData; {}] = [",
        tokens.len()
    )
    .unwrap();
    for token in tokens {
        let context = format!("{TOKEN_DIR}/{}/data.json", token.id);
        let data = &token.data;

        writeln!(
            out,
            "    TokenData {{\n        id: TokenId::{},\n        name: {:?},\n        symbol: {:?},\n        \
             decimals: {},\n        coingecko_id: {:?},\n        native: {},\n        \
             logo_ext: LogoExt::{},\n        description: {:?},\n        website: {:?},\n        \
             twitter: {:?},\n        tokens: &[",
            token.ident,
            required(data, "name", &context, Value::as_str),
            required(data, "symbol", &context, Value::as_str),
            required(data, "decimals", &context, as_u8),
            required(data, "coingeckoId", &context, Value::as_str),
            optional(data, "native", &context, Value::as_bool).unwrap_or(false),
            token.logo_ext,
            optional(data, "description", &context, Value::as_str),
            optional(data, "website", &context, Value::as_str),
            optional(data, "twitter", &context, Value::as_str),
        )
        .unwrap();

        // Per-chain and bridge entries are sorted into `Chain::ALL` order:
        // `Map` iterates alphabetically, or in document order when a consumer
        // enables serde_json's `preserve_order`.
        let mut entries: Vec<_> = required(data, "tokens", &context, Value::as_object)
            .iter()
            .collect();
        entries.sort_by_key(|(key, _)| chain_index(chains, key, &context));
        for (key, entry) in entries {
            let context = format!("{context}: tokens.{key}");
            let entry = as_object(entry, &context);
            let address = required(entry, "address", &context, Value::as_str);
            // Solana mints have no checksum; everything else is formatted as
            // in tokenlist.json.
            let address = if key == "solana" {
                address.to_owned()
            } else {
                address::checksum_address(address).unwrap_or_else(|err| panic!("{context}: {err}"))
            };
            // Tron entries must be base58: the form tokenlist.json carries and
            // `Chain::token` looks up. `checksum_address` keeps hex as hex.
            assert!(
                key != "tron" || address.starts_with('T'),
                "{context}: Tron addresses must be base58 (T...), not {address:?}"
            );
            let mut bridge: Vec<(usize, String)> =
                optional(entry, "bridge", &context, Value::as_object)
                    .into_iter()
                    .flatten()
                    .map(|(bridge_key, bridge_address)| {
                        let bridge_address = bridge_address
                            .as_str()
                            .unwrap_or_else(|| panic!("{context}: invalid `bridge.{bridge_key}`"));
                        let index = chain_index(chains, bridge_key, &context);

                        (
                            index,
                            format!("(Chain::{}, {bridge_address:?})", chains[index].ident),
                        )
                    })
                    .collect();
            bridge.sort_by_key(|(index, _)| *index);
            let bridge: Vec<String> = bridge.into_iter().map(|(_, entry)| entry).collect();
            let empty = Object::new();
            let overrides =
                optional(entry, "overrides", &context, Value::as_object).unwrap_or(&empty);
            let overrides_context = format!("{context}.overrides");

            writeln!(
                out,
                "            ChainTokenData {{ chain: Chain::{}, address: {address:?}, name: {:?}, \
                 symbol: {:?}, decimals: {:?}, native: {:?}, bridge: &[{}], overrides: Overrides \
                 {{ name: {:?}, symbol: {:?}, decimals: {:?} }} }},",
                find_chain(chains, key, &context).ident,
                optional(entry, "name", &context, Value::as_str),
                optional(entry, "symbol", &context, Value::as_str),
                optional(entry, "decimals", &context, as_u8),
                optional(entry, "native", &context, Value::as_bool),
                bridge.join(", "),
                optional(overrides, "name", &overrides_context, Value::as_str),
                optional(overrides, "symbol", &overrides_context, Value::as_str),
                optional(overrides, "decimals", &overrides_context, as_u8),
            )
            .unwrap();
        }

        out.push_str("        ],\n    },\n");
    }
    out.push_str("];\n");

    out
}
