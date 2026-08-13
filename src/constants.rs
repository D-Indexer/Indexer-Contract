/// Ledger TTL bump policy: roughly one year at about five seconds per ledger.
pub const PERSISTENT_BUMP_AMOUNT: u32 = 6_312_000;
/// Ledger TTL threshold used before extending persistent entries.
pub const PERSISTENT_LIFETIME_THRESHOLD: u32 = PERSISTENT_BUMP_AMOUNT - 100_000;

/// Minimum accepted Folder name length.
pub const MIN_FOLDER_NAME_LEN: u32 = 3;
/// Maximum accepted Folder name length.
pub const MAX_FOLDER_NAME_LEN: u32 = 64;
/// Minimum accepted IPFS CID length.
pub const MIN_IPFS_CID_LEN: u32 = 10;
/// Maximum accepted IPFS CID length.
pub const MAX_IPFS_CID_LEN: u32 = 128;
/// Minimum accepted template metadata URI length.
pub const MIN_METADATA_URI_LEN: u32 = 10;
/// Maximum accepted template metadata URI length.
pub const MAX_METADATA_URI_LEN: u32 = 256;
