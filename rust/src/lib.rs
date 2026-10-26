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
//! let ethereum = TokenRegistry::get_ref(TokenId::Usdc, ChainId::Ethereum).unwrap();
//! assert_eq!((ethereum.chain_id(), ethereum.decimals), (1, 6));
//! assert_eq!(ethereum.coingecko_id, "usd-coin");
//!
//! let bob = TokenRegistry::get_ref(TokenId::Usdc, ChainId::Bob).unwrap();
//! assert_eq!(bob.symbol(), "USDC.e");
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

// ChainId, TokenId, CHAIN_IDS, RECORDS, DEPLOYED, FIRST, LOGO_WEBP, TEXT and TEXT_ENDS.
include!(concat!(env!("OUT_DIR"), "/generated.rs"));

// `TOKENLIST_BASE_URL` in typescript/config.ts.
const TOKENLIST_BASE_URL: &str =
    "https://raw.githubusercontent.com/bob-collective/tokenlist/refs/heads/main/";

/// String `index` of `TEXT`: chain keys in `ChainId` order, then token IDs in
/// `TokenId` order, then names and symbols.
const fn text(index: usize) -> &'static str {
    let start = if index == 0 {
        0
    } else {
        TEXT_ENDS[index - 1] as usize
    };
    TEXT.split_at(TEXT_ENDS[index] as usize).0.split_at(start).1
}

/// A `TEXT` index, debug-printed as its string.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Text(u16);

impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(text(self.0 as usize), f)
    }
}

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

/// `FromStr` and `Display` through `as_str`; `ALL` is sorted by it.
macro_rules! impl_strings {
    ($($id:ident),*) => {$(
        impl FromStr for $id {
            type Err = ParseIdError;

            fn from_str(s: &str) -> Result<Self, ParseIdError> {
                let found = Self::ALL.binary_search_by(|id| id.as_str().cmp(s));
                found.map(|i| Self::ALL[i]).map_err(|_| ParseIdError)
            }
        }

        impl fmt::Display for $id {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.pad(self.as_str())
            }
        }
    )*};
}

impl_strings!(TokenId, ChainId);

/// A token on one chain: one `tokenlist-overrides.json` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct TokenData {
    /// The token's ID.
    pub id: TokenId,
    /// The chain it is deployed on.
    pub chain: ChainId,
    /// Contract address or Solana mint. Native assets use `data.json`'s:
    /// the zero address, or a system contract such as POL's `0x…1010`.
    pub address: TokenAddress,
    /// Decimals on this chain.
    pub decimals: u8,
    /// Whether the token is this chain's native asset.
    pub native: bool,
    /// CoinGecko API ID used for price feeds.
    pub coingecko_id: &'static str,
    name: Text,
    symbol: Text,
}

impl TokenData {
    /// Numeric chain ID.
    pub const fn chain_id(&self) -> u64 {
        self.chain.chain_id()
    }

    /// Name on this chain, UI overrides applied.
    pub const fn name(&self) -> &'static str {
        text(self.name.0 as usize)
    }

    /// Symbol on this chain, UI overrides applied (e.g. `USDC.e` on BOB).
    pub const fn symbol(&self) -> &'static str {
        text(self.symbol.0 as usize)
    }

    /// Logo URL on GitHub (`getLogoURI` in the npm package).
    pub fn icon_url(&self) -> String {
        let ext = if LOGO_WEBP[self.id as usize] {
            "webp"
        } else {
            "svg"
        };
        format!("{TOKENLIST_BASE_URL}data/tokens/{}/logo.{ext}", self.id)
    }
}

/// Constant-time lookups into the generated token data.
#[derive(Debug)]
pub struct TokenRegistry;

impl TokenRegistry {
    /// Every supported deployment, by token and then by chain, in `ALL` order.
    pub fn all() -> &'static [TokenData] {
        &RECORDS
    }

    /// The token on this chain, if deployed there. Usable in const contexts,
    /// e.g. to define a token's address as a `const`.
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
}

impl TokenId {
    /// The token's ID string, i.e. its `data/tokens/` directory name.
    pub const fn as_str(self) -> &'static str {
        text(ChainId::ALL.len() + self as usize)
    }
}

impl ChainId {
    /// The chain's `data.json` key, e.g. `"op-mainnet"`.
    pub const fn as_str(self) -> &'static str {
        text(self as usize)
    }

    /// The numeric network ID, independent of this enum's ordinal.
    pub const fn chain_id(self) -> u64 {
        CHAIN_IDS[self as usize]
    }
}
