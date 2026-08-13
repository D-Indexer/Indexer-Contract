// Ledger TTL policy: roughly one year at ~5 seconds per ledger.
pub const PERSISTENT_BUMP_AMOUNT: u32 = 6_312_000;
pub const PERSISTENT_LIFETIME_THRESHOLD: u32 = PERSISTENT_BUMP_AMOUNT - 100_000;

// Keep user-provided strings bounded so callers cannot push unbounded payloads
// into ledger storage.
pub const MIN_FOLDER_NAME_LEN: u32 = 3;
pub const MAX_FOLDER_NAME_LEN: u32 = 64;
pub const MIN_IPFS_CID_LEN: u32 = 10;
pub const MAX_IPFS_CID_LEN: u32 = 128;
pub const MIN_METADATA_URI_LEN: u32 = 10;
pub const MAX_METADATA_URI_LEN: u32 = 256;
