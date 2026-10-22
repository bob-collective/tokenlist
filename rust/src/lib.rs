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
//! use gobob_tokenlist::{ChainId, TokenId, TokenRegistry};
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

pub use alloy_primitives::Address;

// ChainId, TokenId, CHAIN_IDS, RECORDS and INDEX.
include!(concat!(env!("OUT_DIR"), "/generated.rs"));

/// A chain-specific token address. Display preserves base58 strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display)]
pub enum TokenAddress {
    /// A typed EVM address, displayed with its checksum.
    #[strum(transparent)]
    Evm(Address),
    /// Bitcoin's native BTC placeholder: the zero address.
    #[strum(transparent)]
    Bitcoin(Address),
    /// A base58 Solana mint.
    #[strum(transparent)]
    Solana(&'static str),
    /// A base58check Tron address.
    #[strum(transparent)]
    Tron(&'static str),
}

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
    /// Every supported deployment, in registry order.
    pub fn all() -> &'static [TokenData] {
        &RECORDS
    }

    /// The token on this chain, if deployed there.
    #[inline]
    pub fn get(id: TokenId, chain: ChainId) -> Option<TokenData> {
        Self::get_ref(id, chain).copied()
    }

    /// Borrow the token's static data without copying it.
    #[inline]
    pub fn get_ref(id: TokenId, chain: ChainId) -> Option<&'static TokenData> {
        // Slots hold the record index plus one; zero means absent.
        let slot = INDEX[id as usize][chain as usize].checked_sub(1)?;
        Some(&RECORDS[usize::from(slot)])
    }

    /// Equivalent to [`Self::get_ref`].
    #[inline]
    pub fn on_chain(id: TokenId, chain: ChainId) -> Option<&'static TokenData> {
        Self::get_ref(id, chain)
    }
}

impl TokenId {
    /// The token's ID string, i.e. its `data/tokens/` directory name.
    pub fn as_str(self) -> &'static str {
        self.into()
    }

    /// This token's data on the chain, if deployed there.
    #[inline]
    pub fn on_chain(self, chain: ChainId) -> Option<TokenData> {
        TokenRegistry::get(self, chain)
    }
}

impl ChainId {
    /// The chain's `data.json` key, e.g. `"op-mainnet"`.
    pub fn as_str(self) -> &'static str {
        self.into()
    }

    /// The numeric network ID, independent of this enum's ordinal.
    pub fn chain_id(self) -> u64 {
        CHAIN_IDS[self as usize]
    }

    /// The token's data on this chain, if deployed there.
    #[inline]
    pub fn get(self, id: TokenId) -> Option<TokenData> {
        TokenRegistry::get(id, self)
    }
}
