use soroban_sdk::{xdr::ToXdr, Bytes, Env, String, Symbol};

use crate::constants::{
    CIDV0_DIGEST_LEN, CIDV0_LEN, CIDV0_MULTIHASH_CODE, CIDV0_MULTIHASH_LEN, CIDV1_ALLOWED_CODECS,
    CIDV1_ALLOWED_DIGEST_LENS, CIDV1_ALLOWED_MULTIHASH_CODES, CIDV1_MULTIBASE_PREFIX,
    CIDV1_VERSION, FOLDER_SUFFIX, MAX_CIDV1_LEN, MAX_FOLDER_NAME_LEN, MAX_IPFS_CID_LEN,
    MAX_METADATA_URI_LEN, METADATA_URI_SCHEME, MIN_CIDV1_LEN, MIN_FOLDER_LABEL_LEN,
    MIN_IPFS_CID_LEN,
};
use crate::errors::ContractError;

/// Copies a Soroban `Bytes` value into a fixed-size stack buffer so the shared
/// validator can run on `&[u8]` without allocating. Inputs longer than the
/// buffer are rejected by the validators themselves (all limits are smaller).
fn with_bytes<R>(bytes: &Bytes, f: impl FnOnce(&[u8]) -> R) -> R {
    const MAX: usize = 256;
    let mut buf = [0u8; MAX];
    let len = bytes.len() as usize;
    if len > MAX {
        return f(&[]);
    }
    for (i, byte) in bytes.iter().enumerate() {
        buf[i] = byte;
    }
    true
}

fn ends_with(bytes: &Bytes, suffix: &[u8]) -> bool {
    let len = bytes.len();
    let Some(offset) = len.checked_sub(suffix.len() as u32) else {
        return false;
    };
    suffix
        .iter()
        .zip(bytes.slice(offset..len).iter())
        .all(|(s, b)| *s == b)
}

fn all_bytes(bytes: &Bytes, pred: fn(u8) -> bool) -> bool {
    for byte in bytes.iter() {
        if !pred(byte) {
            return false;
        }
    }
    true
}

fn is_label_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
}

/// base58btc alphabet used by CIDv0 (no `0`, `O`, `I`, `l`).
fn is_base58_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() && !matches!(byte, b'0' | b'O' | b'I' | b'l')
}

/// Value of a base58btc character, or `None` if it is not in the alphabet.
fn base58_value(byte: u8) -> Option<u32> {
    const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let mut i = 0;
    while i < ALPHABET.len() {
        if ALPHABET[i] == byte {
            return Some(i as u32);
        }
        i += 1;
    }
    None
}

/// Decodes base58btc into `out`, returning the number of decoded bytes.
/// Fails on invalid characters, on overflow of `out`, or on non-canonical
/// leading zero bytes.
fn base58_decode(bytes: &Bytes, out: &mut [u8]) -> Option<u32> {
    let mut written: usize = 0;
    for byte in bytes.iter() {
        let mut carry = base58_value(byte)?;
        let mut i = 0;
        while i < written {
            carry += (out[i] as u32) * 58;
            out[i] = (carry & 0xff) as u8;
            carry >>= 8;
            i += 1;
        }
        while carry > 0 {
            if written >= out.len() {
                return None;
            }
            out[written] = (carry & 0xff) as u8;
            written += 1;
            carry >>= 8;
        }
    }
    // Leading `1`s encode leading zero bytes.
    let mut zeros: usize = 0;
    for byte in bytes.iter() {
        if byte != b'1' {
            break;
        }
        zeros += 1;
    }
    if zeros + written > out.len() {
        return None;
    }
    // `out` holds the little-endian magnitude; shift it up by `zeros`.
    let mut i = written;
    while i > 0 {
        i -= 1;
        out[zeros + i] = out[i];
    }
    let mut i = 0;
    while i < zeros {
        out[i] = 0;
        i += 1;
    }
    Some((zeros + written) as u32)
}

/// Decodes a CIDv0 (`Qm...`, base58btc) and checks that the result is a
/// 34-byte sha2-256 multihash (`0x12 0x20` + 32-byte digest).
fn is_valid_cidv0(bytes: &Bytes) -> bool {
    if bytes.len() != CIDV0_LEN || !all_bytes(bytes, is_base58_byte) {
        return false;
    }
    let mut buf = [0u8; CIDV0_MULTIHASH_LEN as usize];
    let decoded = match base58_decode(bytes, &mut buf) {
        Some(n) => n,
        None => return false,
    };
    decoded == CIDV0_MULTIHASH_LEN
        && buf[0] == CIDV0_MULTIHASH_CODE
        && buf[1] == CIDV0_DIGEST_LEN
}

/// RFC 4648 lowercase base32 alphabet used by CIDv1 multibase prefix `b`.
fn is_base32_lower_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || (b'2'..=b'7').contains(&byte)
}

fn is_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/')
}

/// Value of a lowercase base32 character, or `None` if it is not in the
/// RFC 4648 alphabet.
fn base32_value(byte: u8) -> Option<u8> {
    match byte {
        b'a'..=b'z' => Some(byte - b'a'),
        b'2'..=b'7' => Some(byte - b'2' + 26),
        _ => None,
    }
}

/// Decodes lowercase base32 (no padding) into `out`, returning the number of
/// decoded bytes. Fails on invalid characters or non-canonical trailing bits.
fn base32_decode(bytes: &Bytes, out: &mut [u8]) -> Option<u32> {
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    let mut written: u32 = 0;
    for byte in bytes.iter() {
        let value = base32_value(byte)? as u32;
        acc = (acc << 5) | value;
        bits += 5;
        while bits >= 8 {
            bits -= 8;
            if written as usize >= out.len() {
                return None;
            }
            out[written as usize] = ((acc >> bits) & 0xff) as u8;
            written += 1;
        }
        // Keep only the bits that have not been emitted yet.
        acc &= (1u32 << bits) - 1;
    }
    // Leftover bits must be zero padding for a canonical encoding.
    if bits >= 5 || acc != 0 {
        return None;
    }
    Some(written)
}

/// Reads an unsigned varint (LEB128, at most 9 bytes) from `data` starting at
/// `*pos`, advancing `*pos` past it.
fn read_varint(data: &[u8], pos: &mut usize) -> Option<u64> {
    let mut value: u64 = 0;
    let mut shift: u32 = 0;
    loop {
        if *pos >= data.len() || shift > 63 {
            return None;
        }
        let byte = data[*pos];
        *pos += 1;
        value |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
        shift += 7;
    }
}

/// Structurally validates a base32 CIDv1: multibase prefix, version, codec,
/// multihash code, digest length and total length.
fn is_valid_cidv1(bytes: &Bytes) -> bool {
    let len = bytes.len();
    if !(MIN_CIDV1_LEN..=MAX_CIDV1_LEN).contains(&len)
        || bytes.get(0) != Some(CIDV1_MULTIBASE_PREFIX)
        || !all_bytes(bytes, is_base32_lower_byte)
    {
        return false;
    }
    let payload = bytes.slice(1..len);
    let mut buf = [0u8; 128];
    let decoded = match base32_decode(&payload, &mut buf) {
        Some(n) => n as usize,
        None => return false,
    };
    let data = &buf[..decoded];
    let mut pos = 0usize;
    let version = match read_varint(data, &mut pos) {
        Some(v) => v,
        None => return false,
    };
    if version != CIDV1_VERSION {
        return false;
    }
    let codec = match read_varint(data, &mut pos) {
        Some(c) => c,
        None => return false,
    };
    if !CIDV1_ALLOWED_CODECS.contains(&codec) {
        return false;
    }
    let mh_code = match read_varint(data, &mut pos) {
        Some(c) => c,
        None => return false,
    };
    if !CIDV1_ALLOWED_MULTIHASH_CODES.contains(&mh_code) {
        return false;
    }
    let digest_len = match read_varint(data, &mut pos) {
        Some(l) => l,
        None => return false,
    };
    if !CIDV1_ALLOWED_DIGEST_LENS.contains(&digest_len) {
        return false;
    }
    // The digest must be the only remaining content.
    digest_len == (decoded - pos) as u64
}

/// Decodes and validates a base58btc CIDv0 multihash, or decodes and
/// structurally validates a base32 CIDv1.
fn is_valid_cid(bytes: &Bytes) -> bool {
    if starts_with(bytes, b"Qm") {
        return is_valid_cidv0(bytes);
    }
    if bytes.get(0) == Some(CIDV1_MULTIBASE_PREFIX) {
        return is_valid_cidv1(bytes);
    }
    Err(ContractError::CidBadPrefix)
}

fn is_valid_cid(bytes: &Bytes) -> bool {
    check_cid(bytes).is_ok()
}

/// Returns true when the label has a hyphen as both its 3rd and 4th bytes,
/// which covers the IDNA A-label prefix `xn--` and other ambiguous `ab--c`
/// shapes.
fn has_ambiguous_hyphen_pair(label: &Bytes) -> bool {
    label.get(2) == Some(b'-') && label.get(3) == Some(b'-')
}

/// Returns true when every byte of the label is an ASCII digit.
fn is_all_numeric(label: &Bytes) -> bool {
    !label.is_empty() && all_bytes(label, is_digit_byte)
}

fn is_digit_byte(byte: u8) -> bool {
    byte.is_ascii_digit()
}

/// Validates a Folder name: `<label>.folder`, where the label is at least
/// `MIN_FOLDER_LABEL_LEN` bytes of `[a-z0-9-]` and does not start or end with
/// a hyphen. The whole name is at most `max_len` bytes.
pub fn require_valid_folder_name(
    folder_name: &String,
    max_len: u32,
) -> Result<(), ContractError> {
    let bytes = folder_name.to_bytes();
    let len = bytes.len();
    let suffix_len = FOLDER_SUFFIX.len() as u32;
    if len > max_len
        || len < MIN_FOLDER_LABEL_LEN + suffix_len
        || !ends_with(&bytes, FOLDER_SUFFIX)
    {
        return Err(ContractError::InvalidFolderName);
    }
    let label = bytes.slice(0..len - suffix_len);
    if !all_bytes(&label, is_label_byte) {
        return Err(ContractError::FolderNameBadChars);
    }
    if label.first() == Some(b'-') || label.last() == Some(b'-') {
        return Err(ContractError::FolderNameBadHyphen);
    }
}

/// Validates an IPFS CID (CIDv0 or base32 CIDv1) before it is anchored on-chain.
pub fn require_valid_ipfs_cid(ipfs_cid: &String) -> Result<(), ContractError> {
    if folder_validation::is_valid_cid(&ipfs_cid.to_bytes().to_alloc_vec()) {
        Ok(())
    } else {
        Err(ContractError::InvalidCID)
    }
}

/// Validates a template metadata URI: `ipfs://<cid>` optionally followed by
/// `/<path>` made of URL-safe characters, at most `MAX_METADATA_URI_LEN` bytes.
pub fn require_valid_metadata_uri(metadata_uri: &String) -> Result<(), ContractError> {
    if folder_validation::is_valid_metadata_uri(&metadata_uri.to_bytes().to_alloc_vec()) {
        Ok(())
    } else {
        Err(ContractError::InvalidMetadataURI)
    }
}

/// Validates template IDs exposed through the public API.
pub fn require_valid_template_id(template_id: u32) -> Result<(), ContractError> {
    if template_id == 0 {
        Err(ContractError::InvalidTemplateID)
    } else {
        Ok(())
    }
}

fn is_record_key_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
}

/// Validates a record key: `[a-z0-9_]{1,16}`. The documented allowlist keys
/// (see `docs/STORAGE.md`) all match this pattern.
pub fn require_valid_record_key(env: &Env, key: &Symbol) -> Result<(), ContractError> {
    // ScVal XDR of a symbol: 4-byte type tag, 4-byte big-endian length, then the characters.
    let xdr = key.clone().to_xdr(env);
    if xdr.len() < 8 {
        return Err(ContractError::InvalidRecordKey);
    }
    let mut len: u32 = 0;
    for byte in xdr.slice(4..8).iter() {
        len = (len << 8) | byte as u32;
    }
    if len == 0 || len > MAX_RECORD_KEY_LEN || 8 + len > xdr.len() {
        return Err(ContractError::InvalidRecordKey);
    }
    if !all_bytes(&xdr.slice(8..8 + len), is_record_key_byte) {
        return Err(ContractError::InvalidRecordKey);
    }
    Ok(())
}

/// Validates a record value: 1 to 256 bytes of printable ASCII.
pub fn require_valid_record_value(value: &String) -> Result<(), ContractError> {
    let bytes = value.to_bytes();
    let len = bytes.len();
    if len == 0 || len > MAX_RECORD_VALUE_LEN || !all_bytes(&bytes, |b| (0x20..=0x7e).contains(&b))
    {
        return Err(ContractError::InvalidRecordValue);
    }
    Ok(())
}
