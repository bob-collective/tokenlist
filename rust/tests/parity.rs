//! Checks the generated tables against the JSON files the npm package builds
//! from the same data: build.rs's inputs (`supported-chains.json`,
//! `compressedlist.json`, read here independently) and
//! `tokenlist-overrides.json`.

use std::collections::{HashMap, HashSet};

use serde_json::Value;
use tokenlist::{ChainId, TokenId, TokenRegistry};

fn parse(json: &str) -> Value {
    serde_json::from_str(json).unwrap()
}

/// Numeric chain ID → `ChainId` for supported chains in `supported-chains.json`,
/// checking that exactly those keys parse as `ChainId`s.
fn supported_chains() -> HashMap<u64, ChainId> {
    let mut chains = HashMap::new();
    for (key, entry) in parse(include_str!("../../supported-chains.json"))
        .as_object()
        .unwrap()
    {
        let parsed = key.parse::<ChainId>();
        if entry["evm"] == false && !matches!(key.as_str(), "bitcoin" | "solana" | "tron") {
            assert!(parsed.is_err(), "{key} is unsupported");
            continue;
        }
        let chain = parsed.unwrap_or_else(|_| panic!("{key} is missing from ChainId"));
        let chain_id = entry["chainId"].as_u64().unwrap();
        assert_eq!((chain.as_str(), chain.chain_id()), (key.as_str(), chain_id));
        chains.insert(chain_id, chain);
    }
    assert_eq!(chains.len(), ChainId::ALL.len());
    chains
}

#[test]
fn tokens_match_compressedlist() {
    let chains = supported_chains();
    let mut ids = HashSet::new();

    for (id, entry) in parse(include_str!("../../compressedlist.json"))
        .as_object()
        .unwrap()
    {
        let entry = entry.as_array().unwrap();
        let deployments: Vec<_> = entry[1..]
            .iter()
            .filter_map(|chain| Some((*chains.get(&chain[0].as_u64().unwrap())?, chain)))
            .collect();
        let Ok(token_id) = id.parse::<TokenId>() else {
            assert!(
                deployments.is_empty(),
                "{id} has supported deployments but no TokenId"
            );
            continue;
        };
        assert!(
            !deployments.is_empty(),
            "{id} has no supported deployments but has a TokenId"
        );
        assert_eq!(token_id.as_str(), id);
        ids.insert(token_id);

        let [name, symbol, coingecko_id, logo] = entry[0].as_array().unwrap().as_slice() else {
            panic!("{id}: malformed shared tuple");
        };
        let logo = format!("/{id}/logo.{}", logo.as_str().unwrap());
        for (chain, chain_entry) in &deployments {
            let context = format!("{id} on {chain}");
            let data =
                TokenRegistry::on_chain(token_id, *chain).unwrap_or_else(|| panic!("no {context}"));
            // The name slot holds null when only the symbol is overridden.
            let field = |index| chain_entry.get(index).filter(|value| !value.is_null());
            assert_eq!(data.name, *field(4).unwrap_or(name), "{context}");
            assert_eq!(data.symbol, *field(5).unwrap_or(symbol), "{context}");
            assert_eq!(data.coingecko_id, *coingecko_id, "{context}");
            assert!(data.icon_url.ends_with(&logo), "{context}");
        }

        let records = TokenRegistry::all()
            .iter()
            .filter(|data| data.id == token_id);
        assert_eq!(records.count(), deployments.len(), "{id}");
    }

    assert_eq!(ids.len(), TokenId::ALL.len());
}

#[test]
fn deployments_match_tokenlist_overrides() {
    let chains = supported_chains();
    let mut seen = HashSet::new();

    for entry in parse(include_str!("../../tokenlist-overrides.json"))["tokens"]
        .as_array()
        .unwrap()
    {
        let chain_id = entry["chainId"].as_u64().unwrap();
        let Some(&chain) = chains.get(&chain_id) else {
            continue; // unsupported
        };
        let extensions = &entry["extensions"];
        let id_str = extensions["tokenId"].as_str().unwrap();
        let id: TokenId = id_str
            .parse()
            .unwrap_or_else(|_| panic!("{id_str} is missing from TokenId"));
        let context = format!("{id} on {chain}");
        let data = TokenRegistry::on_chain(id, chain).unwrap_or_else(|| panic!("no {context}"));

        assert_eq!(data.chain_id, chain_id, "{context}");
        assert_eq!(data.address.to_string(), entry["address"], "{context}");
        assert_eq!(data.name, entry["name"], "{context}");
        assert_eq!(data.symbol, entry["symbol"], "{context}");
        assert_eq!(data.decimals, entry["decimals"], "{context}");
        assert_eq!(data.native, extensions["native"], "{context}");
        assert_eq!(data.icon_url, entry["logoURI"], "{context}");
        // Omitted from the list when longer than the schema's 42-character cap.
        if let Some(coingecko_id) = extensions.get("coingeckoId") {
            assert_eq!(data.coingecko_id, *coingecko_id, "{context}");
        }
        assert!(seen.insert((id, chain)), "{context} is listed twice");
    }

    assert_eq!(TokenRegistry::all().len(), seen.len());
}

#[test]
fn deployments_are_keyed_by_their_chain() {
    let chains = supported_chains();
    for data in TokenRegistry::all() {
        assert_eq!(
            TokenRegistry::get(data.id, chains[&data.chain_id]),
            Some(*data)
        );
    }
}

#[test]
fn version_matches_tokenlist() {
    let version = &parse(include_str!("../../tokenlist.json"))["version"];
    let tokenlist = format!(
        "{}.{}.{}",
        version["major"], version["minor"], version["patch"]
    );
    assert_eq!(env!("CARGO_PKG_VERSION"), tokenlist);
    let package = parse(include_str!("../../package.json"));
    assert_eq!(env!("CARGO_PKG_VERSION"), package["version"]);
}
