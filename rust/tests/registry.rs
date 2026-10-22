use gobob_tokenlist::{Address, ChainId, TokenId, TokenRegistry};

#[test]
fn token_ids_round_trip_through_strings() {
    for &id in TokenRegistry::all().keys() {
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
fn unknown_and_non_evm_ids_do_not_parse() {
    // IDs are case-sensitive; BTC, sBTC and TRX only exist on non-EVM chains.
    for id in ["", "usdc", "BTC_PLUS", "BTC", "sBTC", "TRX"] {
        assert!(id.parse::<TokenId>().is_err(), "{id}");
    }
    for key in ["", "Bob", "OpMainnet", "solana", "bitcoin", "tron"] {
        assert!(key.parse::<ChainId>().is_err(), "{key}");
    }
}

#[test]
fn get_and_on_chain_read_all() {
    for (&id, by_chain) in TokenRegistry::all() {
        assert!(!by_chain.is_empty(), "{id}");
        assert!(std::ptr::eq(TokenRegistry::get(id), by_chain));
        for (&chain, metadata) in by_chain {
            assert_eq!(metadata.id, id);
            let on_chain = TokenRegistry::on_chain(id, chain).unwrap();
            assert!(std::ptr::eq(on_chain, metadata));
        }
    }
}

#[test]
fn metadata_resolves_per_chain_values() {
    let usdc = TokenRegistry::get(TokenId::Usdc);

    let ethereum = &usdc[&ChainId::Ethereum];
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

    let bsc = &usdc[&ChainId::BnbSmartChain];
    assert_eq!((bsc.chain_id, bsc.decimals), (56, 18));

    let bob = &usdc[&ChainId::Bob];
    assert_eq!((bob.name, bob.symbol), ("Bridged USDC", "USDC.e"));

    assert_eq!(TokenRegistry::on_chain(TokenId::Pol, ChainId::Bob), None);
}

#[test]
fn native_flags_come_from_the_data() {
    let eth = TokenRegistry::on_chain(TokenId::Eth, ChainId::Ethereum).unwrap();
    assert!(eth.native);
    assert_eq!(eth.address, Address::ZERO);

    let pol = TokenRegistry::on_chain(TokenId::Pol, ChainId::Polygon).unwrap();
    assert!(pol.native);
    assert_eq!(
        pol.address,
        "0x0000000000000000000000000000000000001010"
            .parse::<Address>()
            .unwrap()
    );

    let usdc = TokenRegistry::on_chain(TokenId::Usdc, ChainId::Ethereum).unwrap();
    assert!(!usdc.native);
}
