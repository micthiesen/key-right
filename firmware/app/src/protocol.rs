//! Profile payload shared by the bounded USB console and host validation.
use key_right_core::pca9635::{Profile, PROFILE_LEN};

pub fn decode_profile(hex: &str) -> Result<Profile, &'static str> {
    if hex.len() != PROFILE_LEN * 2 {
        return Err("profile_requires_160_hex_digits");
    }
    let mut bytes = [0; PROFILE_LEN];
    for (out, pair) in bytes.iter_mut().zip(hex.as_bytes().chunks_exact(2)) {
        let digit = |c: u8| match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            b'A'..=b'F' => Some(c - b'A' + 10),
            _ => None,
        };
        *out = digit(pair[0]).ok_or("invalid_hex")? * 16 + digit(pair[1]).ok_or("invalid_hex")?;
    }
    Profile::decode(&bytes).map_err(|_| "invalid_profile")
}
