use core::fmt;

use crate::{Chain, TOKENLIST_BASE_URL, TOKENLIST_MIRROR_URL, TokenId};

const TOKEN_DIR: &str = "data/tokens";
const CHAIN_DIR: &str = "data/chains";

/// File format of a token logo (`data/tokens/<ID>/logo.<ext>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogoExt {
    /// `logo.svg`
    Svg,
    /// `logo.webp`
    Webp,
}

impl LogoExt {
    /// The file extension without the dot.
    pub const fn as_str(self) -> &'static str {
        match self {
            LogoExt::Svg => "svg",
            LogoExt::Webp => "webp",
        }
    }
}

impl fmt::Display for LogoExt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// GitHub-hosted logo URL of a token (`getLogoURI` in the npm package).
///
/// Prefer [`TokenId::logo_uri`], which picks the logo's actual extension.
pub fn logo_uri(token_id: TokenId, ext: LogoExt) -> String {
    format!("{TOKENLIST_BASE_URL}{TOKEN_DIR}/{token_id}/logo.{ext}")
}

/// BOB R2 mirror logo URL of a token (`getMirrorLogoURI` in the npm package).
///
/// Prefer [`TokenId::mirror_logo_uri`], which picks the logo's actual extension.
pub fn mirror_logo_uri(token_id: TokenId, ext: LogoExt) -> String {
    format!("{TOKENLIST_MIRROR_URL}{TOKEN_DIR}/{token_id}/logo.{ext}")
}

impl TokenId {
    /// GitHub-hosted logo URL, as `logoURI` in `tokenlist.json`.
    pub fn logo_uri(self) -> String {
        logo_uri(self, self.data().logo_ext)
    }

    /// BOB R2 mirror logo URL, as `logoURI` in `tokenlist-mirror.json`.
    pub fn mirror_logo_uri(self) -> String {
        mirror_logo_uri(self, self.data().logo_ext)
    }
}

impl Chain {
    /// GitHub-hosted logo URL, as in `chainlist.json`.
    pub fn logo_uri(self) -> Option<String> {
        self.logo_file()
            .map(|file| format!("{TOKENLIST_BASE_URL}{CHAIN_DIR}/{file}"))
    }

    /// BOB R2 mirror logo URL, as in `chainlist-mirror.json`.
    pub fn mirror_logo_uri(self) -> Option<String> {
        self.logo_file()
            .map(|file| format!("{TOKENLIST_MIRROR_URL}{CHAIN_DIR}/{file}"))
    }
}
