use gobob_tokenlist::{Address, TokenId, TokenRegistry};

#[test]
fn ids_round_trip_through_strings() {
    for token in TokenRegistry::all() {
        let id = token.id;
        assert_eq!(id.as_str().parse::<TokenId>(), Ok(id));
        assert_eq!(id.to_string(), id.as_str());
    }
}

#[test]
fn punctuated_ids_keep_their_strings() {
    let cases = [
        (TokenId::BtcPlus, "BTC+"),
        (TokenId::BtcB, "BTC.b"),
        (TokenId::HybridBtcPendle, "HybridBTC.pendle"),
        (TokenId::SolvBtcJup, "SolvBTC.JUP"),
    ];

    for (id, string) in cases {
        assert_eq!(id.as_str(), string);
        assert_eq!(string.parse::<TokenId>(), Ok(id));
    }
}

#[test]
fn unknown_and_non_evm_only_ids_do_not_parse() {
    // IDs are case-sensitive; BTC, sBTC and TRX only exist on non-EVM chains.
    for id in ["", "usdc", "BTC_PLUS", "BTC", "sBTC", "TRX"] {
        assert!(id.parse::<TokenId>().is_err(), "{id}");
    }
}

#[test]
fn get_is_indexed_by_id() {
    for token in TokenRegistry::all() {
        assert!(std::ptr::eq(TokenRegistry::get(token.id), token));
    }
}

#[test]
fn on_chain_resolves_per_chain_values() {
    let usdc = TokenRegistry::get(TokenId::Usdc);
    assert_eq!((usdc.name, usdc.symbol), ("USD Coin", "USDC"));

    let ethereum = TokenRegistry::on_chain(TokenId::Usdc, 1).unwrap();
    assert_eq!((ethereum.symbol, ethereum.decimals), ("USDC", 6));

    let bsc = TokenRegistry::on_chain(TokenId::Usdc, 56).unwrap();
    assert_eq!(bsc.decimals, 18);

    let bob = TokenRegistry::on_chain(TokenId::Usdc, 60808).unwrap();
    assert_eq!((bob.name, bob.symbol), ("Bridged USDC", "USDC.e"));

    // Solana deployment left out.
    assert_eq!(TokenRegistry::on_chain(TokenId::Usdc, 1584368940), None);
}

#[test]
fn native_flags_come_from_the_data() {
    let eth = TokenRegistry::on_chain(TokenId::Eth, 1).unwrap();
    assert!(eth.native);
    assert_eq!(eth.address, Address::ZERO);

    let pol = TokenRegistry::on_chain(TokenId::Pol, 137).unwrap();
    assert!(pol.native);
    assert_eq!(
        pol.address,
        "0x0000000000000000000000000000000000001010"
            .parse::<Address>()
            .unwrap()
    );

    let usdc = TokenRegistry::on_chain(TokenId::Usdc, 1).unwrap();
    assert!(!usdc.native);
}

#[test]
fn by_address_finds_every_deployment() {
    for token in TokenRegistry::all() {
        for deployment in token.deployments {
            let (found, found_deployment) =
                TokenRegistry::by_address(deployment.chain_id, deployment.address).unwrap();
            assert_eq!((found.id, found_deployment), (token.id, deployment));
        }
    }

    let usdc_on_base = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913"
        .parse()
        .unwrap();
    let (token, _) = TokenRegistry::by_address(8453, usdc_on_base).unwrap();
    assert_eq!(token.id, TokenId::Usdc);
    assert_eq!(TokenRegistry::by_address(1, usdc_on_base), None);
}

#[cfg(feature = "serde")]
#[test]
fn serde_uses_id_strings() {
    assert_eq!(
        serde_json::to_string(&TokenId::BtcPlus).unwrap(),
        r#""BTC+""#
    );
    assert_eq!(
        serde_json::from_str::<TokenId>(r#""BTC.b""#).unwrap(),
        TokenId::BtcB
    );
    assert!(serde_json::from_str::<TokenId>(r#""btc.b""#).is_err());
}
