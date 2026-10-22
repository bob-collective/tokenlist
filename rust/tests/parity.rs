//! Checks the generated table against the JSON files the npm package builds
//! from the same `data/`: `compressedlist.json` (build.rs's input, read here
//! independently) and the `tokenlist*.json` lists.

use std::collections::HashSet;

use gobob_tokenlist::{TokenId, TokenRegistry};
use serde_json::Value;

const COMPRESSEDLIST: &str = include_str!("../../compressedlist.json");
const TOKENLIST: &str = include_str!("../../tokenlist.json");
const TOKENLIST_OVERRIDES: &str = include_str!("../../tokenlist-overrides.json");
const TOKENLIST_OVERRIDES_MIRROR: &str = include_str!("../../tokenlist-overrides-mirror.json");

/// Tron, Bitcoin, Solana and Bitcoin signet.
const NON_EVM_CHAIN_IDS: [u64; 4] = [728126428, 1231006505, 1584368940, 1598918400];

fn parse(json: &str) -> Value {
    serde_json::from_str(json).unwrap()
}

/// The `tokens` of a token list, without non-EVM entries.
fn evm_entries(list: &Value) -> Vec<&Value> {
    list["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| !NON_EVM_CHAIN_IDS.contains(&entry["chainId"].as_u64().unwrap()))
        .collect()
}

fn token_id(entry: &Value) -> TokenId {
    let id = entry["extensions"]["tokenId"].as_str().unwrap();
    id.parse()
        .unwrap_or_else(|_| panic!("{id} is in the token list but not in TokenId"))
}

#[test]
fn tokens_match_compressedlist() {
    let compressed = parse(COMPRESSEDLIST);
    let compressed = compressed.as_object().unwrap();
    let mut ids = HashSet::new();

    for (id, entry) in compressed {
        let entry = entry.as_array().unwrap();
        let has_evm_deployment = entry[1..]
            .iter()
            .any(|chain| !NON_EVM_CHAIN_IDS.contains(&chain[0].as_u64().unwrap()));

        let Ok(token_id) = id.parse::<TokenId>() else {
            assert!(
                !has_evm_deployment,
                "{id} has EVM deployments but no TokenId"
            );
            continue;
        };
        assert!(
            has_evm_deployment,
            "{id} has no EVM deployments but has a TokenId"
        );
        ids.insert(token_id);

        let token = TokenRegistry::get(token_id);
        let [name, symbol, coingecko_id, logo] = entry[0].as_array().unwrap().as_slice() else {
            panic!("{id}: malformed shared tuple");
        };
        assert_eq!(token.id, token_id);
        assert_eq!(token.id.as_str(), id);
        assert_eq!(token.name, *name, "{id}");
        assert_eq!(token.symbol, *symbol, "{id}");
        assert_eq!(token.coingecko_id, *coingecko_id, "{id}");
        assert!(
            token
                .icon_url
                .ends_with(&format!("/{id}/logo.{}", logo.as_str().unwrap())),
            "{id}"
        );
    }

    assert_eq!(ids.len(), TokenRegistry::all().len());
}

#[test]
fn deployments_match_tokenlist_overrides() {
    let list = parse(TOKENLIST_OVERRIDES);
    let entries = evm_entries(&list);
    let mut seen = HashSet::new();

    for entry in &entries {
        let id = token_id(entry);
        let chain_id = entry["chainId"].as_u64().unwrap();
        let token = TokenRegistry::get(id);
        let deployment = TokenRegistry::on_chain(id, chain_id)
            .unwrap_or_else(|| panic!("no {id} deployment on chain {chain_id}"));
        let context = format!("{id} on chain {chain_id}");

        assert_eq!(
            deployment.address.to_checksum(None),
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
        assert_eq!(token.icon_url, entry["logoURI"], "{context}");
        // Omitted from the list when longer than the schema's 42-character cap.
        if let Some(coingecko_id) = entry["extensions"].get("coingeckoId") {
            assert_eq!(token.coingecko_id, *coingecko_id, "{context}");
        }
        assert!(seen.insert((id, chain_id)), "{context} is listed twice");
    }

    let deployments: usize = TokenRegistry::all()
        .iter()
        .map(|token| token.deployments.len())
        .sum();
    assert_eq!(deployments, entries.len());
}

#[test]
fn mirror_icon_urls_match_mirror_list() {
    let list = parse(TOKENLIST_OVERRIDES_MIRROR);

    for entry in evm_entries(&list) {
        let id = token_id(entry);
        assert_eq!(
            TokenRegistry::get(id).mirror_icon_url,
            entry["logoURI"],
            "{id}"
        );
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
}
