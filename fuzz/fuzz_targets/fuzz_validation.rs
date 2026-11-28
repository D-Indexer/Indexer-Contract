//! Fuzz target: arbitrary bytes into every input validator.
//!
//! Invariants: no panic, and a validator that accepts a value must also accept
//! it when it is re-validated (determinism / idempotence).
#![no_main]

use libfuzzer_sys::fuzz_target;
use soroban_sdk::{Bytes, Env, String};

use indexer_contract::validation::{
    require_valid_folder_name, require_valid_ipfs_cid, require_valid_metadata_uri,
    require_valid_template_id,
};

fuzz_target!(|data: &[u8]| {
    let env = Env::default();
    let bytes = Bytes::from_slice(&env, data);
    let text = String::from_bytes(&env, data);

    let folder = require_valid_folder_name(&text);
    assert_eq!(folder, require_valid_folder_name(&text));

    let cid = require_valid_ipfs_cid(&text);
    assert_eq!(cid, require_valid_ipfs_cid(&text));

    let uri = require_valid_metadata_uri(&text);
    assert_eq!(uri, require_valid_metadata_uri(&text));

    // A valid CID must also be a valid `ipfs://<cid>` metadata URI.
    if cid.is_ok() {
        let mut prefixed = Bytes::from_slice(&env, b"ipfs://");
        prefixed.append(&bytes);
        let uri = String::from_bytes(&env, &prefixed);
        assert!(require_valid_metadata_uri(&uri).is_ok());
    }

    // Template IDs: only 0 is rejected.
    let mut id_bytes = [0u8; 4];
    for (slot, byte) in id_bytes.iter_mut().zip(data.iter()) {
        *slot = *byte;
    }
    let template_id = u32::from_be_bytes(id_bytes);
    assert_eq!(require_valid_template_id(template_id).is_ok(), template_id != 0);
});
