//! Address helpers mirroring `checksumAddress`, `toEvmAddress` and
//! `toTronAddress` from the npm package.

use core::fmt;

use sha2::Sha256;
use sha3::{Digest, Keccak256};

/// Error returned by the address utilities for malformed addresses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidAddressError {
    address: String,
}

impl InvalidAddressError {
    pub(crate) fn new(address: &str) -> Self {
        Self {
            address: address.to_owned(),
        }
    }

    /// The rejected address.
    pub fn address(&self) -> &str {
        &self.address
    }
}

impl fmt::Display for InvalidAddressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid address: {:?}", self.address)
    }
}

impl std::error::Error for InvalidAddressError {}

// A Tron address is base58check over 21 bytes: the 0x41 prefix + a 20-byte EVM address.
const TRON_ADDRESS_PREFIX: u8 = 0x41;
// Base58 length of those 25 bytes (payload + checksum): every value with the
// 0x41 prefix encodes to exactly 34 characters.
const TRON_ADDRESS_LEN: usize = 34;

fn sha256d_checksum(payload: &[u8]) -> [u8; 4] {
    let hash = Sha256::digest(Sha256::digest(payload));

    [hash[0], hash[1], hash[2], hash[3]]
}

fn decode_tron_address(address: &str) -> Option<[u8; 21]> {
    // Checked first: base58 decoding is quadratic in the input length.
    if address.len() != TRON_ADDRESS_LEN {
        return None;
    }

    let decoded = bs58::decode(address).into_vec().ok()?;
    let (payload, checksum) = decoded.split_at_checked(decoded.len().checked_sub(4)?)?;
    let payload: [u8; 21] = payload.try_into().ok()?;

    (checksum == sha256d_checksum(&payload) && payload[0] == TRON_ADDRESS_PREFIX).then_some(payload)
}

fn encode_tron_address(evm_address: &[u8; 20]) -> String {
    let mut bytes = Vec::with_capacity(25);
    bytes.push(TRON_ADDRESS_PREFIX);
    bytes.extend_from_slice(evm_address);
    bytes.extend_from_slice(&sha256d_checksum(&bytes));

    bs58::encode(bytes).into_string()
}

fn hex_digits(address: &str) -> Option<&str> {
    let digits = address.strip_prefix("0x")?;

    (digits.len() == 40 && digits.bytes().all(|b| b.is_ascii_hexdigit())).then_some(digits)
}

// Decodes 40 hex digits already validated by `hex_digits`.
fn decode_hex(digits: &str) -> [u8; 20] {
    core::array::from_fn(|i| {
        u8::from_str_radix(&digits[2 * i..2 * i + 2], 16).expect("validated hex digits")
    })
}

// EIP-55 checksum of 40 validated hex digits.
fn eip55(digits: &str) -> String {
    let lower = digits.to_ascii_lowercase();
    let hash = Keccak256::digest(lower.as_bytes());
    let mut out = String::with_capacity(42);
    out.push_str("0x");

    for (i, c) in lower.chars().enumerate() {
        let nibble = if i % 2 == 0 {
            hash[i / 2] >> 4
        } else {
            hash[i / 2] & 0x0f
        };
        out.push(if nibble >= 8 {
            c.to_ascii_uppercase()
        } else {
            c
        });
    }

    out
}

// viem `getAddress`: any well-formed address, checksum not validated.
fn checksum_evm_address(address: &str) -> Result<String, InvalidAddressError> {
    hex_digits(address)
        .map(eip55)
        .ok_or_else(|| InvalidAddressError::new(address))
}

// viem `isAddress` in strict mode: well-formed, and either all-lowercase or
// correctly checksummed.
fn is_strict_address(address: &str) -> bool {
    hex_digits(address).is_some_and(|digits| {
        digits.bytes().all(|b| !b.is_ascii_uppercase()) || eip55(digits) == address
    })
}

/// Normalises an address for comparison and display: EVM addresses are
/// EIP-55 checksummed, valid Tron (base58) addresses are returned unchanged.
///
/// Mixed-case input is not required to carry a valid checksum.
///
/// ```
/// use gobob_tokenlist::checksum_address;
///
/// assert_eq!(
///     checksum_address("0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaed").unwrap(),
///     "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
/// );
/// ```
pub fn checksum_address(address: &str) -> Result<String, InvalidAddressError> {
    if address.starts_with('T') && decode_tron_address(address).is_some() {
        return Ok(address.to_owned());
    }

    checksum_evm_address(address)
}

/// Converts a Tron (base58) address to its 20-byte EVM form, returned as
/// lowercase hex. EVM addresses are returned EIP-55 checksummed.
///
/// ```
/// use gobob_tokenlist::to_evm_address;
///
/// assert_eq!(
///     to_evm_address("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t").unwrap(),
///     "0xa614f803b6fd780986a42c78ec9c7f77e6ded13c",
/// );
/// ```
pub fn to_evm_address(address: &str) -> Result<String, InvalidAddressError> {
    if let Some(decoded) = address
        .starts_with('T')
        .then(|| decode_tron_address(address))
        .flatten()
    {
        let hex: String = decoded[1..].iter().map(|b| format!("{b:02x}")).collect();

        return Ok(format!("0x{hex}"));
    }

    checksum_evm_address(address)
}

/// Converts an EVM address to its Tron (base58) form. Anything that is not a
/// valid EVM address (lowercase or correctly checksummed) is returned as is.
///
/// ```
/// use gobob_tokenlist::to_tron_address;
///
/// assert_eq!(
///     to_tron_address("0xa614f803b6fd780986a42c78ec9c7f77e6ded13c"),
///     "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t",
/// );
/// ```
pub fn to_tron_address(address: &str) -> String {
    if !is_strict_address(address) {
        return address.to_owned();
    }

    encode_tron_address(&decode_hex(&address[2..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected values produced by the npm package's utils.ts.
    #[test]
    fn checksum_address_matches_npm_package() {
        let cases = [
            (
                "0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaed",
                "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
            ),
            (
                "0xFB6916095CA1DF60BB79CE92CE3EA74C37C5D359",
                "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359",
            ),
            (
                "0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB",
                "0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB",
            ),
            (
                "0xd1220a0cf47c7b9be7a2e6ba89f429762e7b9adb",
                "0xD1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb",
            ),
            // Wrong checksum is re-checksummed, not rejected.
            (
                "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAeD",
                "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
            ),
            (
                "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t",
                "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t",
            ),
        ];

        for (input, expected) in cases {
            assert_eq!(checksum_address(input).unwrap(), expected, "{input}");
        }

        for input in [
            "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6x",
            "0x123",
            "0X5aaeb6053f3e94c9b9a09f33669435e7ef1beaed",
            "",
        ] {
            assert_eq!(
                checksum_address(input),
                Err(InvalidAddressError::new(input))
            );
        }
    }

    #[test]
    fn to_evm_address_matches_npm_package() {
        assert_eq!(
            to_evm_address("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t").unwrap(),
            "0xa614f803b6fd780986a42c78ec9c7f77e6ded13c"
        );
        assert_eq!(
            to_evm_address("0xa614f803b6fd780986a42c78ec9c7f77e6ded13c").unwrap(),
            "0xa614f803B6FD780986A42c78Ec9c7f77e6DeD13C"
        );
        assert!(to_evm_address("Tnope").is_err());
    }

    #[test]
    fn oversized_tron_input_is_rejected_before_decoding() {
        let long = "T".repeat(1_000_000);

        assert_eq!(decode_tron_address(&long), None);
        assert!(checksum_address(&long).is_err());
        assert!(to_evm_address(&long).is_err());
        // One character short or long of a valid address.
        assert_eq!(
            decode_tron_address("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6"),
            None
        );
        assert_eq!(
            decode_tron_address("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t1"),
            None
        );
    }

    #[test]
    fn to_tron_address_matches_npm_package() {
        let cases = [
            (
                "0xa614f803b6fd780986a42c78ec9c7f77e6ded13c",
                "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t",
            ),
            (
                "0xa614f803B6FD780986A42c78Ec9c7f77e6DeD13C",
                "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t",
            ),
            // Not strictly valid (all-uppercase / bad checksum): unchanged.
            (
                "0xA614F803B6FD780986A42C78EC9C7F77E6DED13C",
                "0xA614F803B6FD780986A42C78EC9C7F77E6DED13C",
            ),
            (
                "0xa614F803b6fd780986a42c78ec9c7f77e6ded13c",
                "0xa614F803b6fd780986a42c78ec9c7f77e6ded13c",
            ),
            (
                "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t",
                "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t",
            ),
            ("not-an-address", "not-an-address"),
        ];

        for (input, expected) in cases {
            assert_eq!(to_tron_address(input), expected, "{input}");
        }
    }
}
