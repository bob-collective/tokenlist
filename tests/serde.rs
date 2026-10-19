#![cfg(feature = "serde")]

use gobob_tokenlist::{Chain, NativeTokenId, TokenId};

#[test]
fn ids_serialize_as_strings() {
    assert_eq!(serde_json::to_string(&TokenId::BTC_b).unwrap(), "\"BTC.b\"");
    assert_eq!(
        serde_json::to_string(&NativeTokenId::ETH).unwrap(),
        "\"ETH\""
    );
    assert_eq!(
        serde_json::to_string(&Chain::OpMainnet).unwrap(),
        "\"op-mainnet\""
    );
}

#[test]
fn ids_deserialize_from_strings() {
    assert_eq!(
        serde_json::from_str::<TokenId>("\"BTC+\"").unwrap(),
        TokenId::BTC_PLUS
    );
    assert_eq!(
        serde_json::from_str::<NativeTokenId>("\"ETH\"").unwrap(),
        NativeTokenId::ETH
    );
    assert_eq!(
        serde_json::from_str::<Chain>("\"bob\"").unwrap(),
        Chain::Bob
    );

    assert!(serde_json::from_str::<TokenId>("\"NOPE\"").is_err());
    assert!(serde_json::from_str::<NativeTokenId>("\"USDC\"").is_err());
}
