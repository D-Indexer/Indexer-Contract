//! `wasm-bindgen` bindings exposing the shared validation rules to JS.

use wasm_bindgen::prelude::*;

/// Returns `true` when `name` is a valid `<label>.folder` Folder name.
#[wasm_bindgen(js_name = isValidFolderName)]
pub fn is_valid_folder_name(name: &str) -> bool {
    folder_validation::is_valid_folder_name(name.as_bytes())
}

/// Returns `true` when `cid` is a valid CIDv0 or base32 CIDv1.
#[wasm_bindgen(js_name = isValidCid)]
pub fn is_valid_cid(cid: &str) -> bool {
    folder_validation::is_valid_cid(cid.as_bytes())
}

/// Returns `true` when `uri` is a valid `ipfs://<cid>[/<path>]` metadata URI.
#[wasm_bindgen(js_name = isValidMetadataUri)]
pub fn is_valid_metadata_uri(uri: &str) -> bool {
    folder_validation::is_valid_metadata_uri(uri.as_bytes())
}
