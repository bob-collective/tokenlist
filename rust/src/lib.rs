//! Authoritative token registry for the BOB ecosystem: the Rust counterpart
//! of the `@gobob/tokenlist` npm package, for EVM, Bitcoin, Solana and Tron tokens.
//!
//! `build.rs` generates [`ChainId`], [`TokenId`] and static token tables from
//! `supported-chains.json` and `compressedlist.json`. Lookups scan only the
//! selected token's deployments, without initialization or allocation. Enum
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

use std::{fmt, ops::Range, str::FromStr};

pub use alloy_primitives::Address;

// ChainId, TokenId, CHAINS, TOKENS and RECORDS.
include!(concat!(env!("OUT_DIR"), "/generated.rs"));

// `TOKENLIST_BASE_URL` in typescript/config.ts.
const TOKENLIST_BASE_URL: &str =
    "https://raw.githubusercontent.com/bob-collective/tokenlist/refs/heads/main/";

/// Metadata indexed by the corresponding `ChainId` ordinal.
struct ChainMetadata {
    key: &'static str,
    chain_id: u64,
}

/// Metadata indexed by the corresponding `TokenId` ordinal.
struct TokenMetadata {
    id: &'static str,
    logo_extension: &'static str,
    deployments: Range<usize>,
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
    ($($id:ident),+ $(,)?) => {
        $(
            impl FromStr for $id {
                type Err = ParseIdError;

                fn from_str(value: &str) -> Result<Self, Self::Err> {
                    Self::ALL
                        .binary_search_by(|id| id.as_str().cmp(value))
                        .map(|index| Self::ALL[index])
                        .map_err(|_| ParseIdError)
                }
            }

            impl fmt::Display for $id {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.pad(self.as_str())
                }
            }
        )+
    };
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
    /// Name on this chain, with any UI override applied.
    pub name: &'static str,
    /// Symbol on this chain, with any UI override applied.
    pub symbol: &'static str,
}

impl TokenData {
    /// Numeric chain ID.
    pub const fn chain_id(&self) -> u64 {
        self.chain.chain_id()
    }

    /// Name on this chain, UI overrides applied.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Symbol on this chain, UI overrides applied (e.g. `USDC.e` on BOB).
    pub const fn symbol(&self) -> &'static str {
        self.symbol
    }

    /// Logo URL on GitHub (`getLogoURI` in the npm package).
    pub fn icon_url(&self) -> String {
        let ext = TOKENS[self.id as usize].logo_extension;
        format!("{TOKENLIST_BASE_URL}data/tokens/{}/logo.{ext}", self.id)
    }
}

/// Const-compatible lookups into the generated token data.
#[derive(Debug)]
pub struct TokenRegistry;

impl TokenRegistry {
    /// Every supported deployment, by token and then by chain, in `ALL` order.
    pub const fn all() -> &'static [TokenData] {
        &RECORDS
    }

    /// The token on this chain, if deployed there. Usable in const contexts,
    /// e.g. to define a token's address as a `const`. Scans only this token's
    /// deployments, without allocation or runtime initialization.
    #[inline]
    pub const fn get_ref(id: TokenId, chain: ChainId) -> Option<&'static TokenData> {
        let deployments = &TOKENS[id as usize].deployments;
        let mut index = deployments.start;
        // Iterators and derived PartialEq are not const-compatible on the MSRV.
        while index < deployments.end {
            let data = &RECORDS[index];
            if data.chain as usize == chain as usize {
                return Some(data);
            }
            index += 1;
        }
        None
    }
}

impl TokenId {
    /// The token's ID string, i.e. its `data/tokens/` directory name.
    pub const fn as_str(self) -> &'static str {
        TOKENS[self as usize].id
    }
}

impl ChainId {
    /// The chain's `data.json` key, e.g. `"op-mainnet"`.
    pub const fn as_str(self) -> &'static str {
        CHAINS[self as usize].key
    }

    /// The numeric network ID, independent of this enum's ordinal.
    pub const fn chain_id(self) -> u64 {
        CHAINS[self as usize].chain_id
    }
}
