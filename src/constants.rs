/// Approximate number of ledgers per day at ~5 seconds per ledger.
pub const DAY_IN_LEDGERS: u32 = 17_280;

/// Persistent entries are extended to ~120 days. This stays below the
/// testnet/mainnet `max_entry_ttl` (~180 days, 3_110_400 ledgers), so the
/// host never has to clamp the request.
pub const PERSISTENT_BUMP_AMOUNT: u32 = 120 * DAY_IN_LEDGERS;
/// Persistent entries are only re-extended once their remaining TTL drops
/// below ~90 days, i.e. at most once every ~30 days per entry.
pub const PERSISTENT_LIFETIME_THRESHOLD: u32 = 90 * DAY_IN_LEDGERS;

/// Contract instance (and code) TTL policy, applied on every state-changing call.
pub const INSTANCE_BUMP_AMOUNT: u32 = 120 * DAY_IN_LEDGERS;
/// Instance TTL threshold below which the instance is re-extended.
pub const INSTANCE_LIFETIME_THRESHOLD: u32 = 90 * DAY_IN_LEDGERS;

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

/// Maximum number of registered templates. Bounds the size of the
/// persistent template ID list returned by `list_templates`.
pub const MAX_TEMPLATES: u32 = 256;

/// Maximum number of admins. The admin list lives in instance storage, which
/// is loaded on every invocation, so it must stay small.
pub const MAX_ADMINS: u32 = 10;
