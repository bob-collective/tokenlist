use gobob_tokenlist::{Address, ChainId, TokenAddress, TokenData, TokenId, TokenRegistry};

#[test]
fn token_ids_round_trip_through_strings() {
    for &id in TokenId::ALL {
        assert_eq!(id.as_str().parse::<TokenId>(), Ok(id));
        assert_eq!(id.to_string(), id.as_str());
    }
}

#[test]
fn punctuated_ids_keep_their_strings() {
    let tokens = [
        (TokenId::BtcPlus, "BTC+"),
        (TokenId::BtcB, "BTC.b"),
        (TokenId::HybridBtcPendle, "HybridBTC.pendle"),
        (TokenId::SolvBtcJup, "SolvBTC.JUP"),
    ];
    for (id, string) in tokens {
        assert_eq!(id.as_str(), string);
        assert_eq!(string.parse::<TokenId>(), Ok(id));
    }

    let chains = [
        (ChainId::OpMainnet, "op-mainnet"),
        (ChainId::BnbSmartChain, "bnb-smart-chain"),
        (ChainId::HyperEvm, "hyper-evm"),
    ];
    for (chain, string) in chains {
        assert_eq!(chain.as_str(), string);
        assert_eq!(chain.to_string(), string);
        assert_eq!(string.parse::<ChainId>(), Ok(chain));
    }
}

#[test]
fn unknown_and_unsupported_ids_do_not_parse() {
    // IDs are case-sensitive; Bitcoin and Signet remain unsupported.
    for id in ["", "usdc", "BTC_PLUS", "BTC", "sBTC"] {
        assert!(id.parse::<TokenId>().is_err(), "{id}");
    }
    for key in ["", "Bob", "OpMainnet", "bitcoin", "signet"] {
        assert!(key.parse::<ChainId>().is_err(), "{key}");
    }
}

#[test]
fn every_pair_matches_the_static_records() {
    for &id in TokenId::ALL {
        for &chain in ChainId::ALL {
            let expected = TokenRegistry::all()
                .iter()
                .find(|data| data.id == id && data.chain_id == chain.chain_id());
            let owned: Option<TokenData> = expected.copied();
            assert_eq!(TokenRegistry::get(id, chain), owned);
            assert_eq!(id.on_chain(chain), owned);
            assert_eq!(chain.get(id), owned);
            assert_eq!(TokenRegistry::get_ref(id, chain), expected);
            assert_eq!(TokenRegistry::on_chain(id, chain), expected);
            if let Some(expected) = expected {
                assert!(std::ptr::eq(
                    TokenRegistry::get_ref(id, chain).unwrap(),
                    expected
                ));
            }
        }
    }
}

#[test]
fn every_enum_variant_has_a_unique_registry_entry() {
    let tokens: std::collections::HashSet<_> =
        TokenRegistry::all().iter().map(|data| data.id).collect();
    assert_eq!(tokens.len(), TokenId::ALL.len());
    assert!(TokenId::ALL.iter().all(|id| tokens.contains(id)));

    let mut chain_ids = std::collections::HashSet::new();
    for &chain in ChainId::ALL {
        assert!(chain_ids.insert(chain.chain_id()));
        assert_eq!(chain.as_str().parse::<ChainId>(), Ok(chain));
        assert_eq!(chain.to_string(), chain.as_str());
    }
}

#[test]
fn metadata_resolves_per_chain_values() {
    let ethereum = TokenId::Usdc.on_chain(ChainId::Ethereum).unwrap();
    assert_eq!(
        (
            ethereum.chain_id,
            ethereum.name,
            ethereum.symbol,
            ethereum.decimals
        ),
        (1, "USD Coin", "USDC", 6)
    );
    assert_eq!(ethereum.coingecko_id, "usd-coin");

    let bsc = TokenId::Usdc.on_chain(ChainId::BnbSmartChain).unwrap();
    assert_eq!((bsc.chain_id, bsc.decimals), (56, 18));

    let bob = ChainId::Bob.get(TokenId::Usdc).unwrap();
    assert_eq!((bob.name, bob.symbol), ("Bridged USDC", "USDC.e"));

    assert_eq!(TokenRegistry::on_chain(TokenId::Pol, ChainId::Bob), None);
}

#[test]
fn native_flags_come_from_the_data() {
    let eth = TokenRegistry::on_chain(TokenId::Eth, ChainId::Ethereum).unwrap();
    assert!(eth.native);
    assert_eq!(eth.address, TokenAddress::Evm(Address::ZERO));

    let pol = TokenRegistry::on_chain(TokenId::Pol, ChainId::Polygon).unwrap();
    assert!(pol.native);
    assert_eq!(
        pol.address,
        TokenAddress::Evm(
            "0x0000000000000000000000000000000000001010"
                .parse::<Address>()
                .unwrap()
        )
    );

    let usdc = TokenRegistry::on_chain(TokenId::Usdc, ChainId::Ethereum).unwrap();
    assert!(!usdc.native);
}

#[test]
fn non_evm_addresses_preserve_their_format() {
    let sol = TokenId::Sol.on_chain(ChainId::Solana).unwrap();
    let trx = TokenId::Trx.on_chain(ChainId::Tron).unwrap();
    let usdt = TokenId::Usdt.on_chain(ChainId::Tron).unwrap();
    assert_eq!(
        sol.address,
        TokenAddress::Solana("So11111111111111111111111111111111111111112")
    );
    assert_eq!(
        trx.address,
        TokenAddress::Tron("T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb")
    );
    assert_eq!(
        usdt.address,
        TokenAddress::Tron("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t")
    );
    assert!(sol.native && trx.native && !usdt.native);
}
