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
            .filter_map(|chain| chains.get(&chain[0].as_u64().unwrap()).copied())
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

        // The rest is checked against tokenlist-overrides.json, which omits
        // CoinGecko IDs over 42 characters.
        for &chain in &deployments {
            let data = TokenRegistry::get_ref(token_id, chain).unwrap();
            assert_eq!(data.coingecko_id, entry[0][2], "{id} on {chain}");
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
        let data = TokenRegistry::get_ref(id, chain).unwrap_or_else(|| panic!("no {context}"));

        assert_eq!(data.chain_id(), chain_id, "{context}");
        assert_eq!(data.address.to_string(), entry["address"], "{context}");
        assert_eq!(data.name(), entry["name"], "{context}");
        assert_eq!(data.symbol(), entry["symbol"], "{context}");
        assert_eq!(data.decimals, entry["decimals"], "{context}");
        assert_eq!(data.native, extensions["native"], "{context}");
        assert_eq!(data.icon_url(), entry["logoURI"], "{context}");
        // Omitted from the list when longer than the schema's 42-character cap.
        if let Some(coingecko_id) = extensions.get("coingeckoId") {
            assert_eq!(data.coingecko_id, *coingecko_id, "{context}");
        }
        assert!(seen.insert((id, chain)), "{context} is listed twice");
    }

    assert_eq!(TokenRegistry::all().len(), seen.len());
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
