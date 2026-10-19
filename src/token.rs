use crate::generated::TOKENS;
use crate::{Chain, LogoExt, TokenId, checksum_address, to_tron_address};

/// A token's source data: the contents of `data/tokens/<ID>/data.json`
/// (`TokenData` in the npm package).
///
/// Addresses are formatted as in `tokenlist.json`: EVM and Tron addresses
/// checksummed, Solana mints unchanged. Bridge addresses are kept verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct TokenData {
    /// The token's ID (its directory name).
    pub id: TokenId,
    /// Default token name.
    pub name: &'static str,
    /// Default token symbol.
    pub symbol: &'static str,
    /// Default token decimals. A chain entry can override it (see
    /// [`ChainTokenData::decimals`]); [`Token::decimals`] has the resolved
    /// per-chain value.
    pub decimals: u8,
    /// CoinGecko API ID used for price feeds.
    pub coingecko_id: &'static str,
    /// Whether the token is a native chain asset (ETH, BNB, ...). A chain
    /// entry can override it (see [`ChainTokenData::native`]); [`Token::native`]
    /// has the resolved per-chain value.
    pub native: bool,
    /// Format of `data/tokens/<ID>/logo.<ext>`.
    pub logo_ext: LogoExt,
    /// Project or token description.
    pub description: Option<&'static str>,
    /// Project website URL.
    pub website: Option<&'static str>,
    /// Project Twitter/X handle.
    pub twitter: Option<&'static str>,
    /// Per-chain entries, in [`Chain::ALL`] order.
    pub tokens: &'static [ChainTokenData],
}

/// One per-chain entry of [`TokenData::tokens`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ChainTokenData {
    /// The chain this entry is on.
    pub chain: Chain,
    /// Token address (EVM/Tron checksummed) or Solana mint.
    pub address: &'static str,
    /// Chain-specific name used in the base token lists.
    pub name: Option<&'static str>,
    /// Chain-specific symbol used in the base token lists.
    pub symbol: Option<&'static str>,
    /// Chain-specific decimals.
    pub decimals: Option<u8>,
    /// Chain-specific native flag.
    pub native: Option<bool>,
    /// Related chain → bridge token address.
    pub bridge: &'static [(Chain, &'static str)],
    /// UI overrides, applied only by [`tokens_with_overrides`].
    pub overrides: Overrides,
}

/// UI-facing per-chain overrides (`tokenlist-overrides.json`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub struct Overrides {
    /// Overridden name.
    pub name: Option<&'static str>,
    /// Overridden symbol.
    pub symbol: Option<&'static str>,
    /// Overridden decimals.
    pub decimals: Option<u8>,
}

/// A token on one chain: one entry of `tokenlist.json` (or
/// `tokenlist-overrides.json`), with chain-specific values resolved.
///
/// `coingecko_id` is always set, even where `tokenlist.json` omits
/// `extensions.coingeckoId` because the ID exceeds the schema's 42-character cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Token {
    /// `extensions.tokenId`
    pub token_id: TokenId,
    /// Chain of this entry; `chainId` is [`Token::chain_id`].
    pub chain: Chain,
    /// Token address (EVM/Tron checksummed) or Solana mint.
    pub address: &'static str,
    /// Resolved name.
    pub name: &'static str,
    /// Resolved symbol.
    pub symbol: &'static str,
    /// Resolved decimals.
    pub decimals: u8,
    /// `extensions.coingeckoId`
    pub coingecko_id: &'static str,
    /// `extensions.native`
    pub native: bool,
    /// `extensions.bridge`: related chain → bridge token address.
    pub bridge: &'static [(Chain, &'static str)],
}

impl Token {
    /// Numeric chain ID (`chainId`).
    pub const fn chain_id(&self) -> u64 {
        self.chain.id()
    }

    /// GitHub-hosted logo URL (`logoURI` in `tokenlist.json`).
    pub fn logo_uri(&self) -> String {
        self.token_id.logo_uri()
    }

    /// BOB R2 mirror logo URL (`logoURI` in `tokenlist-mirror.json`).
    pub fn mirror_logo_uri(&self) -> String {
        self.token_id.mirror_logo_uri()
    }
}

impl Chain {
    /// The [`tokens`] entry with `address` on this chain.
    ///
    /// Solana mints must match exactly. Every other address is normalized
    /// like the token tables (see [`checksum_address`]), so hex addresses
    /// match in any letter case, and Tron addresses also match as 0x-prefixed
    /// hex (the [`to_evm_address`](crate::to_evm_address) form).
    ///
    /// Scans every token: cache the result on hot paths.
    ///
    /// ```
    /// use gobob_tokenlist::{Chain, TokenId};
    ///
    /// let usdc = Chain::Base.token("0x833589fcd6edb6e08f4c7c32d4f71b54bda02913").unwrap();
    /// assert_eq!((usdc.token_id, usdc.decimals), (TokenId::USDC, 6));
    ///
    /// let usdt = Chain::Tron.token("0xa614f803b6fd780986a42c78ec9c7f77e6ded13c").unwrap();
    /// assert_eq!(usdt.address, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t");
    /// ```
    pub fn token(self, address: &str) -> Option<Token> {
        // Bring `address` into the form stored in the token tables, which
        // build.rs checksums on every chain except Solana.
        let address = match self {
            Chain::Solana => address.to_owned(),
            Chain::Tron => to_tron_address(&checksum_address(address).ok()?),
            _ => checksum_address(address).ok()?,
        };

        tokens().find(|token| token.chain == self && token.address == address)
    }
}

impl TokenId {
    /// The token's source data from `data.json`.
    pub fn data(self) -> &'static TokenData {
        &TOKENS[self as usize]
    }
}

impl ChainTokenData {
    fn resolve(&'static self, data: &'static TokenData, overrides: Overrides) -> Token {
        Token {
            token_id: data.id,
            chain: self.chain,
            address: self.address,
            name: overrides.name.or(self.name).unwrap_or(data.name),
            symbol: overrides.symbol.or(self.symbol).unwrap_or(data.symbol),
            decimals: overrides
                .decimals
                .or(self.decimals)
                .unwrap_or(data.decimals),
            coingecko_id: data.coingecko_id,
            native: self.native.unwrap_or(data.native),
            bridge: self.bridge,
        }
    }
}

/// Every token on every chain, with the values of `tokenlist.json`. Sorted by
/// [`TokenId::ALL`], then by [`Chain::ALL`].
///
/// For `tokenlist-bob.json`, filter on `token.chain == Chain::Bob`.
pub fn tokens() -> impl Iterator<Item = Token> {
    TOKENS.iter().flat_map(|data| {
        data.tokens
            .iter()
            .map(move |token| token.resolve(data, Overrides::default()))
    })
}

/// Every token on every chain with UI overrides applied, with the values of
/// `tokenlist-overrides.json`. Same order as [`tokens`].
pub fn tokens_with_overrides() -> impl Iterator<Item = Token> {
    TOKENS.iter().flat_map(|data| {
        data.tokens
            .iter()
            .map(move |token| token.resolve(data, token.overrides))
    })
}
