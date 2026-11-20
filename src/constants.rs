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

/// Required Folder name suffix.
pub const FOLDER_SUFFIX: &[u8] = b".folder";
/// Minimum length of the label before `.folder` (e.g. `bob` in `bob.folder`).
pub const MIN_FOLDER_LABEL_LEN: u32 = 3;
/// Maximum accepted Folder name length, suffix included.
pub const MAX_FOLDER_NAME_LEN: u32 = 64;
/// Exact length of a CIDv0 (`Qm...`, base58btc sha2-256).
pub const CIDV0_LEN: u32 = 46;
/// Decoded length of a CIDv0 multihash: 2-byte header + 32-byte digest.
pub const CIDV0_MULTIHASH_LEN: u32 = 34;
/// Multihash function code for sha2-256, as used by CIDv0.
pub const CIDV0_MULTIHASH_CODE: u8 = 0x12;
/// Multihash digest length for sha2-256, as used by CIDv0.
pub const CIDV0_DIGEST_LEN: u8 = 0x20;
/// Minimum accepted IPFS CID length.
pub const MIN_IPFS_CID_LEN: u32 = 46;
/// Maximum accepted IPFS CID length.
pub const MAX_IPFS_CID_LEN: u32 = 128;
/// CIDv1 multibase prefix for lowercase base32 (`b`).
pub const CIDV1_MULTIBASE_PREFIX: u8 = b'b';
/// CID version accepted for CIDv1 strings.
pub const CIDV1_VERSION: u64 = 1;
/// Allowlisted CIDv1 codecs: `raw` (0x55) and `dag-pb` (0x70).
pub const CIDV1_ALLOWED_CODECS: [u64; 2] = [0x55, 0x70];
/// Allowlisted multihash functions: `identity` (0x00) and `sha2-256` (0x12).
pub const CIDV1_ALLOWED_MULTIHASH_CODES: [u64; 2] = [0x00, 0x12];
/// Allowlisted multihash digest lengths: 32 bytes (sha2-256) and 0 bytes (identity).
pub const CIDV1_ALLOWED_DIGEST_LENS: [u64; 2] = [32, 0];
/// Maximum accepted CIDv1 string length, multibase prefix included.
pub const MAX_CIDV1_LEN: u32 = 128;
/// Minimum accepted CIDv1 string length, multibase prefix included.
pub const MIN_CIDV1_LEN: u32 = 46;
/// Required template metadata URI scheme.
pub const METADATA_URI_SCHEME: &[u8] = b"ipfs://";
/// Maximum accepted template metadata URI length.
pub const MAX_METADATA_URI_LEN: u32 = 256;

/// Maximum number of registered templates. Bounds the size of the
/// persistent template ID list returned by `list_templates`.
pub const MAX_TEMPLATES: u32 = 256;

/// Maximum number of admins. The admin list lives in instance storage, which
/// is loaded on every invocation, so it must stay small.
pub const MAX_ADMINS: u32 = 10;
