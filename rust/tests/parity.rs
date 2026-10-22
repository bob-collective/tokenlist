//! Checks the generated tables against the JSON files the npm package builds
//! from the same data: build.rs's inputs (`supported-chains.json`,
//! `compressedlist.json`, read here independently) and
//! `tokenlist-overrides.json`.

use std::collections::{HashMap, HashSet};

use gobob_tokenlist::{ChainId, TokenId, TokenRegistry};
use serde_json::Value;

const SUPPORTED_CHAINS: &str = include_str!("../../supported-chains.json");
const COMPRESSEDLIST: &str = include_str!("../../compressedlist.json");
const TOKENLIST: &str = include_str!("../../tokenlist.json");
const TOKENLIST_OVERRIDES: &str = include_str!("../../tokenlist-overrides.json");

fn parse(json: &str) -> Value {
    serde_json::from_str(json).unwrap()
}

/// Numeric chain ID → `ChainId` for supported chains in `supported-chains.json`,
/// checking that exactly those keys parse as `ChainId`s.
fn supported_chains() -> HashMap<u64, ChainId> {
    let mut chains = HashMap::new();

    for (key, entry) in parse(SUPPORTED_CHAINS).as_object().unwrap() {
        let parsed = key.parse::<ChainId>();
        if entry["evm"] == false && !matches!(key.as_str(), "solana" | "tron") {
            assert!(parsed.is_err(), "{key} is unsupported");
            continue;
        }
        let chain = parsed.unwrap_or_else(|_| panic!("{key} is missing from ChainId"));
        assert_eq!(chain.as_str(), key);
        assert_eq!(chain.chain_id(), entry["chainId"].as_u64().unwrap());
        chains.insert(entry["chainId"].as_u64().unwrap(), chain);
    }

    assert_eq!(chains.len(), ChainId::ALL.len());
    chains
}

#[test]
fn tokens_match_compressedlist() {
    let chains = supported_chains();
    let mut ids = HashSet::new();

    for (id, entry) in parse(COMPRESSEDLIST).as_object().unwrap() {
        let entry = entry.as_array().unwrap();
        let has_supported_deployment = entry[1..]
            .iter()
            .any(|chain| chains.contains_key(&chain[0].as_u64().unwrap()));

        let Ok(token_id) = id.parse::<TokenId>() else {
            assert!(
                !has_supported_deployment,
                "{id} has supported deployments but no TokenId"
            );
            continue;
        };
        assert!(
            has_supported_deployment,
            "{id} has no supported deployments but has a TokenId"
        );
        ids.insert(token_id);

        assert_eq!(token_id.as_str(), id);

        let [name, symbol, coingecko_id, logo] = entry[0].as_array().unwrap().as_slice() else {
            panic!("{id}: malformed shared tuple");
        };
        let logo = format!("/{id}/logo.{}", logo.as_str().unwrap());
        let mut chain_count = 0;

        for chain_entry in &entry[1..] {
            let Some(&chain) = chains.get(&chain_entry[0].as_u64().unwrap()) else {
                continue; // unsupported
            };
            chain_count += 1;
            let context = format!("{id} on {chain}");
            let metadata =
                TokenRegistry::on_chain(token_id, chain).unwrap_or_else(|| panic!("no {context}"));
            // The name slot holds null when only the symbol is overridden.
            let name_override = chain_entry.get(4).filter(|name| !name.is_null());
            let symbol_override = chain_entry.get(5).filter(|symbol| !symbol.is_null());

            assert_eq!(metadata.name, *name_override.unwrap_or(name), "{context}");
            assert_eq!(
                metadata.symbol,
                *symbol_override.unwrap_or(symbol),
                "{context}"
            );
            assert_eq!(metadata.coingecko_id, *coingecko_id, "{context}");
            assert!(metadata.icon_url.ends_with(&logo), "{context}");
        }

        assert_eq!(
            TokenRegistry::all()
                .iter()
                .filter(|data| data.id == token_id)
                .count(),
            chain_count,
            "{id}"
        );
    }

    assert_eq!(ids.len(), TokenId::ALL.len());
}

#[test]
fn deployments_match_tokenlist_overrides() {
    let chains = supported_chains();
    let mut seen = HashSet::new();

    for entry in parse(TOKENLIST_OVERRIDES)["tokens"].as_array().unwrap() {
        let chain_id = entry["chainId"].as_u64().unwrap();
        let Some(&chain) = chains.get(&chain_id) else {
            continue; // unsupported
        };
        let id_str = entry["extensions"]["tokenId"].as_str().unwrap();
        let id: TokenId = id_str
            .parse()
            .unwrap_or_else(|_| panic!("{id_str} is missing from TokenId"));
        let context = format!("{id} on {chain}");
        let deployment =
            TokenRegistry::on_chain(id, chain).unwrap_or_else(|| panic!("no {context}"));

        assert_eq!(deployment.chain_id, chain_id, "{context}");
        assert_eq!(
            deployment.address.to_string(),
            entry["address"],
            "{context}"
        );
        assert_eq!(deployment.name, entry["name"], "{context}");
        assert_eq!(deployment.symbol, entry["symbol"], "{context}");
        assert_eq!(
            u64::from(deployment.decimals),
            entry["decimals"],
            "{context}"
        );
        assert_eq!(
            deployment.native, entry["extensions"]["native"],
            "{context}"
        );
        assert_eq!(deployment.icon_url, entry["logoURI"], "{context}");
        // Omitted from the list when longer than the schema's 42-character cap.
        if let Some(coingecko_id) = entry["extensions"].get("coingeckoId") {
            assert_eq!(deployment.coingecko_id, *coingecko_id, "{context}");
        }
        assert!(seen.insert((id, chain)), "{context} is listed twice");
    }

    assert_eq!(TokenRegistry::all().len(), seen.len());
}

#[test]
fn deployments_are_keyed_by_their_chain() {
    let chains = supported_chains();

    for metadata in TokenRegistry::all() {
        let chain = chains[&metadata.chain_id];
        assert_eq!(TokenRegistry::get(metadata.id, chain), Some(*metadata));
    }
}

#[test]
fn version_matches_tokenlist() {
    let version = &parse(TOKENLIST)["version"];
    let tokenlist_version = format!(
        "{}.{}.{}",
        version["major"], version["minor"], version["patch"]
    );

    assert_eq!(env!("CARGO_PKG_VERSION"), tokenlist_version);
    let package = parse(include_str!("../../package.json"));
    assert_eq!(
        env!("CARGO_PKG_VERSION"),
        package["version"].as_str().unwrap()
    );
}
