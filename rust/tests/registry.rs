use std::collections::HashSet;
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

#[test]
fn punctuated_ids_keep_their_strings() {
    let tokens = [
        TokenId::BtcPlus,
        TokenId::BtcB,
        TokenId::HybridBtcPendle,
        TokenId::SolvBtcJup,
    ];
    let expected = ["BTC+", "BTC.b", "HybridBTC.pendle", "SolvBTC.JUP"];
    assert_eq!(tokens.map(TokenId::as_str), expected);

    let chains = [
        ChainId::OpMainnet,
        ChainId::BnbSmartChain,
        ChainId::HyperEvm,
    ];
    let expected = ["op-mainnet", "bnb-smart-chain", "hyper-evm"];
    assert_eq!(chains.map(ChainId::as_str), expected);
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
                .find(|data| data.id == id && data.chain_id == chain.chain_id());
            for borrowed in [
                TokenRegistry::get_ref(id, chain),
                TokenRegistry::on_chain(id, chain),
            ] {
                assert_eq!(borrowed.map(ptr::from_ref), expected.map(ptr::from_ref));
            }
            for owned in [
                TokenRegistry::get(id, chain),
                id.on_chain(chain),
                chain.get(id),
            ] {
                assert_eq!(owned, expected.copied());
            }
        }
    }
}

#[test]
fn borrowed_lookups_are_const() {
    const USDC: Option<&TokenData> = TokenRegistry::get_ref(TokenId::Usdc, ChainId::Ethereum);
    const POL: Option<&TokenData> = TokenRegistry::on_chain(TokenId::Pol, ChainId::Bob);
    assert_eq!(USDC.map(|data| data.decimals), Some(6));
    assert_eq!(POL, None);
}

#[test]
fn every_token_is_deployed_and_every_chain_id_unique() {
    let tokens: HashSet<_> = TokenRegistry::all().iter().map(|data| data.id).collect();
    assert_eq!(tokens, TokenId::ALL.iter().copied().collect());
    let chain_ids: HashSet<_> = ChainId::ALL.iter().map(|chain| chain.chain_id()).collect();
    assert_eq!(chain_ids.len(), ChainId::ALL.len());
}

#[test]
fn addresses_and_native_flags_come_from_the_data() {
    let evm = |address: &str| TokenAddress::Evm(address.parse().unwrap());
    for (id, chain, address, native) in [
        (
            TokenId::Btc,
            ChainId::Bitcoin,
            TokenAddress::Bitcoin(Address::ZERO),
            true,
        ),
        (
            TokenId::Eth,
            ChainId::Ethereum,
            TokenAddress::Evm(Address::ZERO),
            true,
        ),
        (
            TokenId::Pol,
            ChainId::Polygon,
            evm("0x0000000000000000000000000000000000001010"),
            true,
        ),
        (
            TokenId::Usdc,
            ChainId::Ethereum,
            evm("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            false,
        ),
        (
            TokenId::Sol,
            ChainId::Solana,
            TokenAddress::Solana("So11111111111111111111111111111111111111112"),
            true,
        ),
        (
            TokenId::Trx,
            ChainId::Tron,
            TokenAddress::Tron("T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb"),
            true,
        ),
        (
            TokenId::Usdt,
            ChainId::Tron,
            TokenAddress::Tron("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"),
            false,
        ),
    ] {
        let data = TokenRegistry::get(id, chain).unwrap();
        assert_eq!(
            (data.address, data.native),
            (address, native),
            "{id} on {chain}"
        );
    }

    let btc = TokenId::Btc.on_chain(ChainId::Bitcoin).unwrap();
    assert_eq!((btc.chain_id, ChainId::Bitcoin.chain_id()), (90000, 90000));
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
        (TokenId::OUsdt, ChainId::Base, 6),
        (TokenId::Usdt, ChainId::Solana, 6),
    ] {
        let data = TokenRegistry::get(id, chain).unwrap();
        assert_eq!(data.decimals, decimals, "{id} on {chain}");
    }
}

#[test]
fn robinhood_stocks_keep_their_coingecko_ids_and_decimals() {
    let expected = [
        (TokenId::Nvda, "nvidia-robinhood-tokenized-stock"),
        (TokenId::Spcx, "spacex-robinhood-tokenized-stock"),
        (
            TokenId::Amc,
            "amc-entertainment-holdings-inc-robinhood-token",
        ),
        (TokenId::Gme, "gamestop-robinhood-tokenized-stock"),
        (TokenId::Googl, "alphabet-class-a-robinhood-tokenized-stock"),
        (TokenId::Aapl, "apple-robinhood-tokenized-stock"),
        (TokenId::Tsla, "tesla-robinhood-tokenized-stock"),
        (
            TokenId::Djt,
            "trump-media-technology-group-corp-robinhood-token",
        ),
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
        (
            TokenId::Spy,
            "spdr-s-p-500-etf-trust-robinhood-tokenized-stock",
        ),
        (
            TokenId::Sgov,
            "ishares-0-3-month-treasury-bond-etf-robinhood-tokenized-stock",
        ),
        (TokenId::Qqq, "invesco-qqq-robinhood-tokenized-stock"),
        (
            TokenId::Slv,
            "ishares-silver-trust-robinhood-tokenized-stock",
        ),
        (TokenId::Gld, "spdr-gold-shares-robinhood-token"),
        (
            TokenId::Ibm,
            "international-business-machines-corporation-robinhood-token",
        ),
        (TokenId::Lulu, "lululemon-robinhood-tokenized-stock"),
        (TokenId::Ups, "ups-robinhood-tokenized-stock"),
        (TokenId::Cost, "costco-robinhood-tokenized-stock"),
        (
            TokenId::Crcl,
            "circle-internet-group-robinhood-tokenized-stock",
        ),
        (
            TokenId::Ttwo,
            "take-two-interactive-software-robinhood-tokenized-stock",
        ),
        (TokenId::Baba, "alibaba-robinhood-tokenized-stock"),
        (TokenId::Fig, "figma-inc-robinhood-token"),
        (TokenId::Nflx, "netflix-robinhood-tokenized-stock"),
        (
            TokenId::Pltr,
            "palantir-technologies-robinhood-tokenized-stock",
        ),
        (TokenId::Coin, "coinbase-robinhood-tokenized-stock"),
        (
            TokenId::Sndk,
            "sandisk-corporation-robinhood-tokenized-stock",
        ),
        (TokenId::Amd, "amd-robinhood-tokenized-stock"),
        (TokenId::Qubt, "quantum-computing-robinhood-tokenized-stock"),
        (TokenId::Shop, "shopify-robinhood-tokenized-stock"),
        (
            TokenId::Tsm,
            "taiwan-semiconductor-manufacturing-robinhood-tokenized-stock",
        ),
        (TokenId::Glxy, "galaxy-digital-inc-robinhood-token"),
        (TokenId::Intc, "intel-robinhood-tokenized-stock"),
        (
            TokenId::Uso,
            "united-states-oil-fund-robinhood-tokenized-stock",
        ),
        (TokenId::Inda, "ishares-msci-india-etf-robinhood-token"),
    ];
    for (id, coingecko_id) in expected {
        let data = TokenRegistry::get(id, ChainId::RobinhoodChain).unwrap();
        assert_eq!(
            (data.coingecko_id, data.decimals),
            (coingecko_id, 18),
            "{id}"
        );
    }
}
