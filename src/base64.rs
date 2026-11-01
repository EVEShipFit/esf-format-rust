//! The binary form in URLs: base64url without padding (SPEC §10.4).

use crate::Error;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// The binary form as base64url without padding, for URLs (§10.4).
pub fn to_base64url(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let bytes = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let bits = u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]]);
        for at in 0..=chunk.len() {
            out.push(ALPHABET[(bits >> (18 - 6 * at) & 0x3F) as usize] as char);
        }
    }
    out
}

fn sextet(c: u8) -> Option<u32> {
    ALPHABET.iter().position(|&a| a == c).map(|at| at as u32)
}

/// The binary form from base64url without padding (§10.4).
pub fn from_base64url(encoded: &str) -> Result<Vec<u8>, Error> {
    let encoded = encoded.trim().as_bytes();
    let invalid = || Error::new("not valid base64url");
    if encoded.len() % 4 == 1 {
        return Err(invalid());
    }
    let mut out = Vec::with_capacity(encoded.len() / 4 * 3 + 2);
    for chunk in encoded.chunks(4) {
        let mut bits = 0;
        for (at, &c) in chunk.iter().enumerate() {
            bits |= sextet(c).ok_or_else(invalid)? << (18 - 6 * at);
        }
        out.extend_from_slice(&bits.to_be_bytes()[1..chunk.len()]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        for len in 0..=7 {
            let data: Vec<u8> = (0..len)
                .map(|n: u8| n.wrapping_mul(37).wrapping_add(250))
                .collect();
            assert_eq!(from_base64url(&to_base64url(&data)).unwrap(), data);
        }
    }

    #[test]
    fn known_values() {
        assert_eq!(to_base64url(b""), "");
        assert_eq!(to_base64url(b"f"), "Zg");
        assert_eq!(to_base64url(b"fo"), "Zm8");
        assert_eq!(to_base64url(b"foo"), "Zm9v");
        assert_eq!(to_base64url(&[0xFB, 0xFF]), "-_8");
        assert_eq!(from_base64url(" Zm9vYg\n").unwrap(), b"foob");
    }

    #[test]
    fn rejects() {
        assert!(from_base64url("Zm9vY").is_err());
        assert!(from_base64url("Zm9v=").is_err());
        assert!(from_base64url("Zm+v").is_err());
        assert!(from_base64url("not base64url!").is_err());
    }
}
