//! Pure byte-slice validators. They have no Soroban dependencies so they can
//! be verified with Kani (`cargo kani`); `validation.rs` holds thin adapters
//! that copy contract `String`s into a buffer and call these functions.

use crate::constants::{
    CIDV0_LEN, FOLDER_SUFFIX, MAX_FOLDER_NAME_LEN, MAX_IPFS_CID_LEN, MAX_METADATA_URI_LEN,
    METADATA_URI_SCHEME, MIN_FOLDER_LABEL_LEN, MIN_IPFS_CID_LEN,
};

pub fn is_label_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
}

/// base58btc alphabet used by CIDv0 (no `0`, `O`, `I`, `l`).
pub fn is_base58_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() && !matches!(byte, b'0' | b'O' | b'I' | b'l')
}

/// RFC 4648 lowercase base32 alphabet used by CIDv1 multibase prefix `b`.
pub fn is_base32_lower_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || (b'2'..=b'7').contains(&byte)
}

fn is_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/')
}

/// Checks the textual shape of a CIDv0 (`Qm` + base58btc, 46 chars) or a
/// base32 CIDv1 (`ba` + lowercase base32, 46–128 chars).
pub fn is_valid_cid(bytes: &[u8]) -> bool {
    let len = bytes.len();
    if bytes.starts_with(b"Qm") {
        return len == CIDV0_LEN as usize && bytes.iter().all(|&b| is_base58_byte(b));
    }
    if bytes.starts_with(b"ba") {
        return (MIN_IPFS_CID_LEN as usize..=MAX_IPFS_CID_LEN as usize).contains(&len)
            && bytes.iter().all(|&b| is_base32_lower_byte(b));
    }
    false
}

/// `<label>.folder`, where the label is at least `MIN_FOLDER_LABEL_LEN` bytes
/// of `[a-z0-9-]` and does not start or end with a hyphen. The whole name is
/// at most `MAX_FOLDER_NAME_LEN` bytes.
pub fn is_valid_folder_name(bytes: &[u8]) -> bool {
    let len = bytes.len();
    let suffix_len = FOLDER_SUFFIX.len();
    if len > MAX_FOLDER_NAME_LEN as usize
        || len < MIN_FOLDER_LABEL_LEN as usize + suffix_len
        || !bytes.ends_with(FOLDER_SUFFIX)
    {
        return false;
    }
    let label = &bytes[..len - suffix_len];
    label.iter().all(|&b| is_label_byte(b))
        && label.first() != Some(&b'-')
        && label.last() != Some(&b'-')
}

/// `ipfs://<cid>` optionally followed by `/<path>` made of URL-safe
/// characters, at most `MAX_METADATA_URI_LEN` bytes.
pub fn is_valid_metadata_uri(bytes: &[u8]) -> bool {
    if bytes.len() > MAX_METADATA_URI_LEN as usize || !bytes.starts_with(METADATA_URI_SCHEME) {
        return false;
    }
    let rest = &bytes[METADATA_URI_SCHEME.len()..];
    let (cid, path) = match rest.iter().position(|&b| b == b'/') {
        Some(end) => rest.split_at(end),
        None => (rest, &[][..]),
    };
    is_valid_cid(cid) && path.iter().all(|&b| is_path_byte(b))
}

#[cfg(kani)]
mod proofs {
    use super::*;

    /// Folder-name inputs are bounded to 16 bytes (label up to 9 bytes).
    const NAME_BOUND: usize = 16;
    /// CID inputs are bounded to 48 bytes, which covers the full CIDv0 length
    /// (46) and the shortest CIDv1 lengths.
    const CID_BOUND: usize = 48;
    /// Metadata URI inputs are bounded to 24 bytes (panic-freedom only).
    const URI_BOUND: usize = 24;

    fn any_slice<const N: usize>(buf: &[u8; N]) -> &[u8] {
        let len: usize = kani::any();
        kani::assume(len <= N);
        &buf[..len]
    }

    #[kani::proof]
    #[kani::unwind(18)]
    fn folder_name_never_panics_and_matches_grammar() {
        let buf: [u8; NAME_BOUND] = kani::any();
        let name = any_slice(&buf);
        if is_valid_folder_name(name) {
            let n = name.len();
            assert!(n >= MIN_FOLDER_LABEL_LEN as usize + FOLDER_SUFFIX.len());
            assert!(n <= MAX_FOLDER_NAME_LEN as usize);
            assert!(&name[n - FOLDER_SUFFIX.len()..] == FOLDER_SUFFIX);
            let label = &name[..n - FOLDER_SUFFIX.len()];
            let mut i = 0;
            while i < label.len() {
                let b = label[i];
                assert!(b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
                i += 1;
            }
            // Label edges are never hyphens.
            assert!(label[0] != b'-');
            assert!(label[label.len() - 1] != b'-');
        }
    }

    #[kani::proof]
    #[kani::unwind(50)]
    fn cid_never_panics_and_has_valid_shape() {
        let buf: [u8; CID_BOUND] = kani::any();
        let cid = any_slice(&buf);
        if is_valid_cid(cid) {
            let n = cid.len();
            if cid[0] == b'Q' {
                assert!(cid[1] == b'm');
                assert!(n == CIDV0_LEN as usize);
                let mut i = 0;
                while i < n {
                    assert!(is_base58_byte(cid[i]));
                    i += 1;
                }
            } else {
                assert!(cid[0] == b'b' && cid[1] == b'a');
                assert!(n >= MIN_IPFS_CID_LEN as usize && n <= MAX_IPFS_CID_LEN as usize);
                let mut i = 0;
                while i < n {
                    assert!(is_base32_lower_byte(cid[i]));
                    i += 1;
                }
            }
        }
    }

    #[kani::proof]
    #[kani::unwind(26)]
    fn metadata_uri_never_panics() {
        let buf: [u8; URI_BOUND] = kani::any();
        let uri = any_slice(&buf);
        if is_valid_metadata_uri(uri) {
            assert!(uri.starts_with(METADATA_URI_SCHEME));
        }
    }
}
