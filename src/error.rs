use core::fmt;

use crate::TokenId;

/// Error returned when parsing a [`Chain`](crate::Chain), a [`TokenId`] or a
/// token ID subset (e.g. [`NativeTokenId`](crate::NativeTokenId)) from an
/// unknown string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    kind: &'static str,
    value: String,
}

impl ParseError {
    pub(crate) fn new(kind: &'static str, value: &str) -> Self {
        Self {
            kind,
            value: value.to_owned(),
        }
    }

    /// The string that failed to parse.
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown {}: {:?}", self.kind, self.value)
    }
}

impl std::error::Error for ParseError {}

/// Error returned when converting a [`TokenId`] into a subset it is not part
/// of (e.g. `NativeTokenId::try_from(TokenId::USDC)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TryFromTokenIdError {
    target: &'static str,
    token_id: TokenId,
}

impl TryFromTokenIdError {
    pub(crate) fn new(target: &'static str, token_id: TokenId) -> Self {
        Self { target, token_id }
    }

    /// The token ID that failed to convert.
    pub fn token_id(&self) -> TokenId {
        self.token_id
    }
}

impl fmt::Display for TryFromTokenIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} is not a {}", self.token_id, self.target)
    }
}

impl std::error::Error for TryFromTokenIdError {}
