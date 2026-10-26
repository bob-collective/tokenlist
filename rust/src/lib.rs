//! Authoritative token registry for the BOB ecosystem: the Rust counterpart
//! of the `@gobob/tokenlist` npm package, for EVM, Bitcoin, Solana and Tron tokens.
//!
//! `build.rs` generates [`ChainId`], [`TokenId`] and static token tables from
//! `supported-chains.json` and `compressedlist.json`, so lookups are
//! constant-time, without runtime initialization, allocation or hashing. Enum
//! ordinals are internal indices, not stable IDs; use strings for storage and
//! [`ChainId::chain_id`] for numeric network IDs.
//!
//! ```
//! use tokenlist::{ChainId, TokenId, TokenRegistry};
//!
//! let ethereum = TokenRegistry::get(TokenId::Usdc, ChainId::Ethereum).unwrap();
//! assert_eq!((ethereum.chain_id, ethereum.decimals), (1, 6));
//! assert_eq!(ethereum.coingecko_id, "usd-coin");
//!
//! let bsc = TokenId::Usdc.on_chain(ChainId::BnbSmartChain).unwrap();
//! assert_eq!(bsc.decimals, 18);
//!
//! let bob = ChainId::Bob.get(TokenId::Usdc).unwrap();
//! assert_eq!(bob.symbol, "USDC.e");
//!
//! let id: TokenId = "BTC+".parse().unwrap();
//! assert_eq!(id, TokenId::BtcPlus);
//! assert_eq!(id.as_str(), "BTC+");
//! assert_eq!("op-mainnet".parse(), Ok(ChainId::OpMainnet));
//! ```
//!
//! Signet is not covered; `sBTC` has no [`TokenId`].

#![warn(missing_docs)]

use std::{fmt, str::FromStr};

pub use alloy_primitives::Address;

// ChainId, TokenId, CHAIN_IDS, CHAIN_KEYS, TOKEN_KEYS, RECORDS, DEPLOYED and FIRST.
include!(concat!(env!("OUT_DIR"), "/generated.rs"));

/// A chain-specific token address. Display preserves base58 strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenAddress {
    /// A typed EVM address, displayed with its checksum.
    Evm(Address),
    /// Bitcoin's native BTC placeholder: the zero address.
    Bitcoin(Address),
    /// A base58 Solana mint.
    Solana(&'static str),
    /// A base58check Tron address.
    Tron(&'static str),
}

impl fmt::Display for TokenAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Evm(address) | Self::Bitcoin(address) => fmt::Display::fmt(address, f),
            Self::Solana(address) | Self::Tron(address) => f.pad(address),
        }
    }
}

/// An unknown [`TokenId`] or [`ChainId`] string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseIdError;

impl fmt::Display for ParseIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown token or chain ID")
    }
}

impl std::error::Error for ParseIdError {}

/// `FromStr` and `Display` through each enum's sorted key table.
macro_rules! impl_keys {
    ($($id:ident $keys:ident),*) => {$(
        impl FromStr for $id {
            type Err = ParseIdError;

            fn from_str(s: &str) -> Result<Self, ParseIdError> {
                $keys.binary_search(&s).map(|i| Self::ALL[i]).map_err(|_| ParseIdError)
            }
        }

        impl fmt::Display for $id {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.pad(self.as_str())
            }
        }
    )*};
}

impl_keys!(TokenId TOKEN_KEYS, ChainId CHAIN_KEYS);

/// A token on one chain: one `tokenlist-overrides.json` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct TokenData {
    /// The token's ID.
    pub id: TokenId,
    /// Numeric chain ID.
    pub chain_id: u64,
    /// Contract address or Solana mint. Native assets use `data.json`'s:
    /// the zero address, or a system contract such as POL's `0x…1010`.
    pub address: TokenAddress,
    /// Decimals on this chain.
    pub decimals: u8,
    /// Whether the token is this chain's native asset.
    pub native: bool,
    /// Name on this chain, UI overrides applied.
    pub name: &'static str,
    /// Symbol on this chain, UI overrides applied (e.g. `USDC.e` on BOB).
    pub symbol: &'static str,
    /// CoinGecko API ID used for price feeds.
    pub coingecko_id: &'static str,
    /// Logo URL on GitHub (`getLogoURI` in the npm package).
    pub icon_url: &'static str,
}

/// Compatibility name for [`TokenData`].
pub type TokenMetadata = TokenData;

/// Constant-time lookups into the generated token data.
#[derive(Debug)]
pub struct TokenRegistry;

impl TokenRegistry {
    /// Every supported deployment, by token and then by chain, in `ALL` order.
    pub fn all() -> &'static [TokenData] {
        &RECORDS
    }

    /// The token on this chain, if deployed there.
    #[inline]
    pub fn get(id: TokenId, chain: ChainId) -> Option<TokenData> {
        Self::get_ref(id, chain).copied()
    }

    /// Borrow the token's static data without copying it. Usable in const
    /// contexts, e.g. to define a token's address as a `const`.
    #[inline]
    pub const fn get_ref(id: TokenId, chain: ChainId) -> Option<&'static TokenData> {
        // A token's records follow ChainId order from FIRST, so skip one per
        // deployment on a lower ordinal.
        let (deployed, bit) = (DEPLOYED[id as usize], 1 << chain as u64);
        if deployed & bit == 0 {
            return None;
        }
        let before = (deployed & (bit - 1)).count_ones() as usize;
        Some(&RECORDS[FIRST[id as usize] as usize + before])
    }

    /// Equivalent to [`Self::get_ref`].
    #[inline]
    pub const fn on_chain(id: TokenId, chain: ChainId) -> Option<&'static TokenData> {
        Self::get_ref(id, chain)
    }
}

impl TokenId {
    /// The token's ID string, i.e. its `data/tokens/` directory name.
    pub const fn as_str(self) -> &'static str {
        TOKEN_KEYS[self as usize]
    }

    /// This token's data on the chain, if deployed there.
    #[inline]
    pub fn on_chain(self, chain: ChainId) -> Option<TokenData> {
        TokenRegistry::get(self, chain)
    }
}

impl ChainId {
    /// The chain's `data.json` key, e.g. `"op-mainnet"`.
    pub const fn as_str(self) -> &'static str {
        CHAIN_KEYS[self as usize]
    }

    /// The numeric network ID, independent of this enum's ordinal.
    pub const fn chain_id(self) -> u64 {
        CHAIN_IDS[self as usize]
    }

    /// The token's data on this chain, if deployed there.
    #[inline]
    pub fn get(self, id: TokenId) -> Option<TokenData> {
        TokenRegistry::get(id, self)
    }
}
