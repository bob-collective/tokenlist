use std::ptr;

use tokenlist::{Address, ChainId, TokenAddress, TokenData, TokenId, TokenRegistry};

#[test]
fn ids_round_trip_through_strings() {
    for &id in TokenId::ALL {
        assert_eq!(id.as_str().parse(), Ok(id));
        assert_eq!(id.to_string(), id.as_str());
    }
    for &chain in ChainId::ALL {
        assert_eq!(chain.as_str().parse(), Ok(chain));
        assert_eq!(chain.to_string(), chain.as_str());
    }
}

/// Variants keep their original strings. Besides the first row of punctuated
/// IDs, the tokens are bob-gateway's `TokenSymbol` names: renaming one breaks
/// its mapping. Its Robinhood equities are pinned below.
#[test]
fn variants_keep_their_strings() {
    #[rustfmt::skip]
    let tokens = [
        (TokenId::BtcPlus, "BTC+"), (TokenId::HybridbtcDotPendle, "HybridBTC.pendle"), (TokenId::SolvbtcDotJup, "SolvBTC.JUP"),
        (TokenId::Eth, "ETH"), (TokenId::Weth, "WETH"), (TokenId::Wbtc, "WBTC"), (TokenId::Usdt, "USDT"),
        (TokenId::Usdc, "USDC"), (TokenId::Bnb, "BNB"), (TokenId::Wbnb, "WBNB"), (TokenId::Avax, "AVAX"),
        (TokenId::Wavax, "WAVAX"), (TokenId::Btc, "BTC"), (TokenId::Paxg, "PAXG"), (TokenId::Xaut, "XAUt"),
        (TokenId::Xaut0, "XAUt0"), (TokenId::Cbbtc, "cbBTC"), (TokenId::Btcb, "BTCB"), (TokenId::Dai, "DAI"),
        (TokenId::Eurc, "EURC"), (TokenId::Link, "LINK"), (TokenId::Pyusd, "PYUSD"), (TokenId::Jpyc, "JPYC"),
        (TokenId::Tbtc, "tBTC"), (TokenId::Lbtc, "LBTC"), (TokenId::BtcDotB, "BTC.b"), (TokenId::Solvbtc, "SolvBTC"),
        (TokenId::Usd1, "USD1"), (TokenId::Rlusd, "RLUSD"), (TokenId::Fidd, "FIDD"), (TokenId::Usdg, "USDG"),
        (TokenId::Bob, "BOB"),
    ];
    for (id, expected) in tokens {
        assert_eq!(id.as_str(), expected);
    }
    #[rustfmt::skip]
    let chains = [
        (ChainId::OpMainnet, "op-mainnet"), (ChainId::BnbSmartChain, "bnb-smart-chain"), (ChainId::HyperEvm, "hyper-evm"),
    ];
    for (chain, expected) in chains {
        assert_eq!(chain.as_str(), expected);
    }
}

#[test]
fn unknown_and_unsupported_ids_do_not_parse() {
    // IDs are case-sensitive; Signet remains unsupported.
    for id in ["", "usdc", "BTC_PLUS", "sBTC"] {
        assert!(id.parse::<TokenId>().is_err(), "{id}");
    }
    for key in ["", "Bob", "OpMainnet", "signet"] {
        assert!(key.parse::<ChainId>().is_err(), "{key}");
    }
}

#[test]
fn every_pair_matches_the_static_records() {
    for &id in TokenId::ALL {
        for &chain in ChainId::ALL {
            let expected = TokenRegistry::all()
                .iter()
                .find(|data| data.id == id && data.chain == chain);
            let found = TokenRegistry::get_ref(id, chain);
            assert_eq!(found.map(ptr::from_ref), expected.map(ptr::from_ref));
        }
    }
}

/// bob-gateway's use, which must keep compiling: const lookups of EVM
/// addresses and decimals, and `coingecko_id` as a field.
#[test]
fn gateway_contract_is_const() {
    const fn deployment(id: TokenId, chain: ChainId) -> &'static TokenData {
        match TokenRegistry::get_ref(id, chain) {
            Some(data) => data,
            None => panic!("not deployed"),
        }
    }
    const fn address(id: TokenId, chain: ChainId) -> Address {
        match deployment(id, chain).address {
            TokenAddress::Evm(address) => address,
            _ => panic!("not an EVM token"),
        }
    }
    const USDC: (Address, u8) = (
        address(TokenId::Usdc, ChainId::Ethereum),
        deployment(TokenId::Usdc, ChainId::Ethereum).decimals,
    );
    const STRINGS: [&str; 3] = [
        TokenId::BtcPlus.as_str(),
        ChainId::Bob.as_str(),
        deployment(TokenId::Usdc, ChainId::Bob).symbol(),
    ];
    const BOB: u64 = ChainId::Bob.chain_id();
    const POL: Option<&TokenData> = TokenRegistry::get_ref(TokenId::Pol, ChainId::Bob);
    let usdc = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
        .parse()
        .unwrap();
    assert_eq!(USDC, (usdc, 6));
    assert_eq!(STRINGS, ["BTC+", "bob", "USDC.e"]);
    assert_eq!((BOB, POL), (60808, None));
    let usdc = deployment(TokenId::Usdc, ChainId::Ethereum);
    assert_eq!(usdc.coingecko_id, "usd-coin");
}

#[test]
fn addresses_and_native_flags_come_from_the_data() {
    let evm = |address: &str| TokenAddress::Evm(address.parse().unwrap());
    #[rustfmt::skip]
    let cases = [
        (TokenId::Btc, ChainId::Bitcoin, TokenAddress::Bitcoin(Address::ZERO), true),
        (TokenId::Eth, ChainId::Ethereum, TokenAddress::Evm(Address::ZERO), true),
        (TokenId::Pol, ChainId::Polygon, evm("0x0000000000000000000000000000000000001010"), true),
        (TokenId::Usdc, ChainId::Ethereum, evm("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"), false),
        (TokenId::Sol, ChainId::Solana, TokenAddress::Solana("So11111111111111111111111111111111111111112"), true),
        (TokenId::Trx, ChainId::Tron, TokenAddress::Tron("T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb"), true),
        (TokenId::Usdt, ChainId::Tron, TokenAddress::Tron("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"), false),
    ];
    for (id, chain, address, native) in cases {
        let data = TokenRegistry::get_ref(id, chain).unwrap();
        assert_eq!(
            (data.address, data.native),
            (address, native),
            "{id} on {chain}"
        );
    }

    let btc = TokenRegistry::get_ref(TokenId::Btc, ChainId::Bitcoin).unwrap();
    assert_eq!(
        (btc.chain_id(), ChainId::Bitcoin.chain_id()),
        (90000, 90000)
    );
    assert_eq!((btc.decimals, btc.coingecko_id), (8, "bitcoin"));
}

#[test]
fn decimals_come_from_the_data() {
    for (id, chain, decimals) in [
        (TokenId::Wbtc, ChainId::Ethereum, 8),
        (TokenId::Wbtcoft, ChainId::Bob, 8),
        (TokenId::Usdt, ChainId::Ethereum, 6),
        (TokenId::Usdc, ChainId::Ethereum, 6),
        (TokenId::Usdc, ChainId::Base, 6),
        (TokenId::Ousdt, ChainId::Base, 6),
        (TokenId::Usdt, ChainId::Solana, 6),
    ] {
        let data = TokenRegistry::get_ref(id, chain).unwrap();
        assert_eq!(data.decimals, decimals, "{id} on {chain}");
    }
}

#[test]
fn robinhood_stocks_keep_their_coingecko_ids_and_decimals() {
    #[rustfmt::skip]
    let expected = [
        (TokenId::Nvda, "nvidia-robinhood-tokenized-stock"),
        (TokenId::Spcx, "spacex-robinhood-tokenized-stock"),
        (TokenId::Amc, "amc-entertainment-holdings-inc-robinhood-token"),
        (TokenId::Gme, "gamestop-robinhood-tokenized-stock"),
        (TokenId::Googl, "alphabet-class-a-robinhood-tokenized-stock"),
        (TokenId::Aapl, "apple-robinhood-tokenized-stock"),
        (TokenId::Tsla, "tesla-robinhood-tokenized-stock"),
        (TokenId::Djt, "trump-media-technology-group-corp-robinhood-token"),
        (TokenId::Mstr, "strategy-inc-robinhood-tokenized-stock"),
        (TokenId::Lly, "eli-lilly-robinhood-tokenized-stock"),
        (TokenId::Hims, "hims-hers-health-inc-robinhood-token"),
        (TokenId::Mu, "micron-technology-robinhood-tokenized-stock"),
        (TokenId::Snap, "snap-inc-robinhood-token"),
        (TokenId::Rivn, "rivian-automotive-robinhood-tokenized-stock"),
        (TokenId::Amzn, "amazon-robinhood-tokenized-stock"),
        (TokenId::Msft, "microsoft-robinhood-tokenized-stock"),
        (TokenId::Rddt, "reddit-robinhood-tokenized-stock"),
        (TokenId::Rblx, "roblox-robinhood-tokenized-stock"),
        (TokenId::Meta, "meta-platforms-robinhood-tokenized-stock"),
        (TokenId::Spy, "spdr-s-p-500-etf-trust-robinhood-tokenized-stock"),
        (TokenId::Sgov, "ishares-0-3-month-treasury-bond-etf-robinhood-tokenized-stock"),
        (TokenId::Qqq, "invesco-qqq-robinhood-tokenized-stock"),
        (TokenId::Slv, "ishares-silver-trust-robinhood-tokenized-stock"),
        (TokenId::Gld, "spdr-gold-shares-robinhood-token"),
        (TokenId::Ibm, "international-business-machines-corporation-robinhood-token"),
        (TokenId::Lulu, "lululemon-robinhood-tokenized-stock"),
        (TokenId::Ups, "ups-robinhood-tokenized-stock"),
        (TokenId::Cost, "costco-robinhood-tokenized-stock"),
        (TokenId::Crcl, "circle-internet-group-robinhood-tokenized-stock"),
        (TokenId::Ttwo, "take-two-interactive-software-robinhood-tokenized-stock"),
        (TokenId::Baba, "alibaba-robinhood-tokenized-stock"),
        (TokenId::Fig, "figma-inc-robinhood-token"),
        (TokenId::Nflx, "netflix-robinhood-tokenized-stock"),
        (TokenId::Pltr, "palantir-technologies-robinhood-tokenized-stock"),
        (TokenId::Coin, "coinbase-robinhood-tokenized-stock"),
        (TokenId::Sndk, "sandisk-corporation-robinhood-tokenized-stock"),
        (TokenId::Amd, "amd-robinhood-tokenized-stock"),
        (TokenId::Qubt, "quantum-computing-robinhood-tokenized-stock"),
        (TokenId::Shop, "shopify-robinhood-tokenized-stock"),
        (TokenId::Tsm, "taiwan-semiconductor-manufacturing-robinhood-tokenized-stock"),
        (TokenId::Glxy, "galaxy-digital-inc-robinhood-token"),
        (TokenId::Intc, "intel-robinhood-tokenized-stock"),
        (TokenId::Uso, "united-states-oil-fund-robinhood-tokenized-stock"),
        (TokenId::Inda, "ishares-msci-india-etf-robinhood-token"),
    ];
    for (id, coingecko_id) in expected {
        let data = TokenRegistry::get_ref(id, ChainId::RobinhoodChain).unwrap();
        assert_eq!(
            (data.coingecko_id, data.decimals),
            (coingecko_id, 18),
            "{id}"
        );
    }
}
