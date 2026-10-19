//! The generated JSON files, embedded verbatim. Same content as the files
//! the npm package exports (`@gobob/tokenlist/tokenlist.json`, ...).

/// `tokenlist.json`: every token on every chain.
pub const TOKENLIST: &str = include_str!("../tokenlist.json");
/// `tokenlist-bob.json`: BOB-chain entries only.
pub const TOKENLIST_BOB: &str = include_str!("../tokenlist-bob.json");
/// `tokenlist-overrides.json`: full list with per-chain UI overrides applied.
pub const TOKENLIST_OVERRIDES: &str = include_str!("../tokenlist-overrides.json");
/// `tokenlist-mirror.json`: [`TOKENLIST`] with R2 mirror logo URIs.
pub const TOKENLIST_MIRROR: &str = include_str!("../tokenlist-mirror.json");
/// `tokenlist-bob-mirror.json`: [`TOKENLIST_BOB`] with R2 mirror logo URIs.
pub const TOKENLIST_BOB_MIRROR: &str = include_str!("../tokenlist-bob-mirror.json");
/// `tokenlist-overrides-mirror.json`: [`TOKENLIST_OVERRIDES`] with R2 mirror logo URIs.
pub const TOKENLIST_OVERRIDES_MIRROR: &str = include_str!("../tokenlist-overrides-mirror.json");
/// `chainlist.json`: chain ID → GitHub-hosted logo URI.
pub const CHAINLIST: &str = include_str!("../chainlist.json");
/// `chainlist-mirror.json`: chain ID → R2 mirror logo URI.
pub const CHAINLIST_MIRROR: &str = include_str!("../chainlist-mirror.json");
/// `supported-chains.json`: chain key → chain ID, name, EVM and testnet flags.
pub const SUPPORTED_CHAINS: &str = include_str!("../supported-chains.json");
/// `compressedlist.json`: size-optimized token list (see the README).
pub const COMPRESSEDLIST: &str = include_str!("../compressedlist.json");
