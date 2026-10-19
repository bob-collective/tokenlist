//! Checks the Rust data, which build.rs reads from `data/`, against the JSON
//! files `pnpm build` generates from it, so the two cannot drift: run
//! `pnpm build` after changing `data/`, then `cargo test`.

use std::collections::{BTreeMap, BTreeSet};

use gobob_tokenlist::{
    Chain, NativeTokenId, NonEvmNativeTokenId, NonEvmTokenId, SUPPORTED_CHAIN_IDS,
    TOKENLIST_SCHEMA_URL, Token, TokenId, checksum_address, json, to_evm_address, to_tron_address,
    tokens, tokens_with_overrides,
};
use serde_json::{Map, Value, json};

// The Uniswap tokenlist schema caps extension values at 42 characters, so
// longer CoinGecko IDs are left out of the JSON lists.
const MAX_EXTENSION_LENGTH: usize = 42;

fn parse(source: &str) -> Value {
    serde_json::from_str(source).unwrap()
}

fn token_json(token: &Token, logo_uri: String) -> Value {
    let bridge: Map<String, Value> = token
        .bridge
        .iter()
        .map(|(chain, address)| (chain.id().to_string(), json!(address)))
        .collect();
    let mut extensions = json!({
        "tokenId": token.token_id.as_str(),
        "native": token.native,
        "bridge": bridge,
    });

    if token.coingecko_id.len() <= MAX_EXTENSION_LENGTH {
        extensions["coingeckoId"] = json!(token.coingecko_id);
    }

    json!({
        "chainId": token.chain_id(),
        "address": token.address,
        "name": token.name,
        "symbol": token.symbol,
        "decimals": token.decimals,
        "logoURI": logo_uri,
        "extensions": extensions,
    })
}

// Entries keyed by (token ID, chain ID): the JSON lists are sorted with
// JavaScript's `localeCompare`, the Rust tables by lowercase token ID.
fn keyed(entries: impl IntoIterator<Item = Value>) -> BTreeMap<(String, u64), Value> {
    let mut keyed = BTreeMap::new();

    for entry in entries {
        let key = (
            entry["extensions"]["tokenId"].as_str().unwrap().to_owned(),
            entry["chainId"].as_u64().unwrap(),
        );

        assert!(
            keyed.insert(key.clone(), entry).is_none(),
            "duplicate {key:?}"
        );
    }

    keyed
}

fn assert_list_matches(
    list: &str,
    tokens: impl Iterator<Item = Token>,
    logo_uri: fn(&Token) -> String,
) {
    let expected = keyed(parse(list)["tokens"].as_array().unwrap().clone());
    let actual = keyed(tokens.map(|token| token_json(&token, logo_uri(&token))));

    assert_eq!(
        actual.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>()
    );
    for (key, actual) in &actual {
        assert_eq!(actual, &expected[key]);
    }
}

#[test]
fn tokens_match_tokenlist() {
    assert_list_matches(json::TOKENLIST, tokens(), Token::logo_uri);
    assert_list_matches(json::TOKENLIST_MIRROR, tokens(), Token::mirror_logo_uri);
}

#[test]
fn bob_tokens_match_bob_tokenlist() {
    let bob_tokens = || tokens().filter(|token| token.chain == Chain::Bob);

    assert_list_matches(json::TOKENLIST_BOB, bob_tokens(), Token::logo_uri);
    assert_list_matches(
        json::TOKENLIST_BOB_MIRROR,
        bob_tokens(),
        Token::mirror_logo_uri,
    );
}

#[test]
fn tokens_with_overrides_match_overrides_tokenlist() {
    assert_list_matches(
        json::TOKENLIST_OVERRIDES,
        tokens_with_overrides(),
        Token::logo_uri,
    );
    assert_list_matches(
        json::TOKENLIST_OVERRIDES_MIRROR,
        tokens_with_overrides(),
        Token::mirror_logo_uri,
    );
}

#[test]
fn chain_logos_match_chainlist() {
    for (list, logo_uri) in [
        (
            json::CHAINLIST,
            Chain::logo_uri as fn(Chain) -> Option<String>,
        ),
        (json::CHAINLIST_MIRROR, Chain::mirror_logo_uri),
    ] {
        let expected = parse(list);
        let actual: Map<String, Value> = Chain::ALL
            .iter()
            .filter_map(|chain| Some((chain.id().to_string(), json!(logo_uri(*chain)?))))
            .collect();

        assert_eq!(Value::Object(actual), expected);
    }
}

#[test]
fn token_ids_match_compressedlist() {
    let compressed = parse(json::COMPRESSEDLIST);
    let expected: BTreeSet<&str> = compressed
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let actual: BTreeSet<&str> = TokenId::ALL.iter().map(|id| id.as_str()).collect();

    assert_eq!(actual, expected);
    assert_eq!(TokenId::ALL.len(), actual.len());
}

#[test]
fn token_data_is_indexed_by_token_id() {
    for &id in TokenId::ALL {
        assert_eq!(id.data().id, id);
        assert_eq!(id.as_str().parse::<TokenId>(), Ok(id));
    }
}

#[test]
fn token_id_subsets_match_token_data() {
    let native: Vec<TokenId> = TokenId::ALL
        .iter()
        .copied()
        .filter(|id| id.data().native)
        .collect();
    let non_evm: Vec<TokenId> = TokenId::ALL
        .iter()
        .copied()
        .filter(|id| id.data().tokens.iter().any(|token| !token.chain.is_evm()))
        .collect();
    let non_evm_native: Vec<TokenId> = non_evm
        .iter()
        .copied()
        .filter(|id| id.data().native)
        .collect();

    assert_eq!(
        NativeTokenId::ALL
            .iter()
            .map(|&id| id.into())
            .collect::<Vec<TokenId>>(),
        native
    );
    assert_eq!(
        NonEvmTokenId::ALL
            .iter()
            .map(|&id| id.into())
            .collect::<Vec<TokenId>>(),
        non_evm
    );
    assert_eq!(
        NonEvmNativeTokenId::ALL
            .iter()
            .map(|&id| id.into())
            .collect::<Vec<TokenId>>(),
        non_evm_native
    );

    for &id in TokenId::ALL {
        assert_eq!(NativeTokenId::try_from(id).is_ok(), native.contains(&id));
    }
}

#[test]
fn chains_round_trip() {
    assert_eq!(SUPPORTED_CHAIN_IDS.len(), Chain::ALL.len());

    for (&chain, &id) in Chain::ALL.iter().zip(SUPPORTED_CHAIN_IDS) {
        assert_eq!(chain.id(), id);
        assert_eq!(Chain::from_id(id), Some(chain));
        assert_eq!(chain.as_str().parse::<Chain>(), Ok(chain));
    }
}

#[test]
fn addresses_are_checksummed() {
    for token in tokens() {
        if token.chain == Chain::Solana {
            continue;
        }

        assert_eq!(
            checksum_address(token.address).as_deref(),
            Ok(token.address)
        );

        if token.chain == Chain::Tron {
            let evm = to_evm_address(token.address).unwrap();
            assert_eq!(to_tron_address(&evm), token.address);
        }
    }
}

#[test]
fn chain_token_finds_every_token() {
    let mut seen = BTreeSet::new();

    for token in tokens() {
        assert!(
            seen.insert((token.chain, token.address)),
            "{} is listed twice on {}",
            token.address,
            token.chain
        );
        assert_eq!(token.chain.token(token.address), Some(token));

        if token.chain.is_evm() {
            let lowercase = token.address.to_ascii_lowercase();
            assert_eq!(token.chain.token(&lowercase), Some(token));
        }
        if token.chain == Chain::Tron {
            let evm = to_evm_address(token.address).unwrap();
            assert_eq!(Chain::Tron.token(&evm), Some(token));
            assert_eq!(
                Chain::Tron.token(&evm.to_ascii_uppercase().replacen("0X", "0x", 1)),
                Some(token)
            );
        }
    }

    let usdc_on_base = Chain::Base.token("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913");
    assert_eq!(
        usdc_on_base.map(|token| token.token_id),
        Some(TokenId::USDC)
    );
    assert_eq!(
        Chain::Bob.token("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
        None
    );
    assert_eq!(Chain::Bob.token("not-an-address"), None);
    let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    assert_eq!(
        Chain::Solana.token(usdc_mint).map(|token| token.token_id),
        Some(TokenId::USDC)
    );
    // Solana mints are case-sensitive base58.
    assert_eq!(Chain::Solana.token(&usdc_mint.to_ascii_lowercase()), None);
}

#[test]
fn version_and_schema_match_tokenlist() {
    let tokenlist = parse(json::TOKENLIST);
    let version = &tokenlist["version"];

    assert_eq!(
        format!(
            "{}.{}.{}",
            version["major"], version["minor"], version["patch"]
        ),
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(tokenlist["$schema"], TOKENLIST_SCHEMA_URL);
}

#[test]
fn parse_errors() {
    let error = "NOPE".parse::<TokenId>().unwrap_err();
    assert_eq!(error.to_string(), "unknown token id: \"NOPE\"");
    assert_eq!(error.value(), "NOPE");

    let error = NativeTokenId::try_from(TokenId::USDC).unwrap_err();
    assert_eq!(error.to_string(), "USDC is not a NativeTokenId");
    assert_eq!(error.token_id(), TokenId::USDC);

    assert!("USDC".parse::<NativeTokenId>().is_err());
    assert_eq!(
        "ETH".parse::<NativeTokenId>().map(TokenId::from),
        Ok(TokenId::ETH)
    );
    assert!("mars".parse::<Chain>().is_err());
}
