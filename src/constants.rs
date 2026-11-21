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
/// Minimum accepted IPFS CID length.
pub const MIN_IPFS_CID_LEN: u32 = 46;
/// Maximum accepted IPFS CID length.
pub const MAX_IPFS_CID_LEN: u32 = 128;
/// Required template metadata URI scheme.
pub const METADATA_URI_SCHEME: &[u8] = b"ipfs://";
/// Maximum accepted template metadata URI length.
pub const MAX_METADATA_URI_LEN: u32 = 256;

/// Maximum number of registered templates. Bounds the size of the
/// persistent template ID list returned by `list_templates`.
pub const MAX_TEMPLATES: u32 = 256;

/// Contract version reported by `version()`. Bump on every upgrade that
/// changes behavior or storage layout.
pub const CONTRACT_VERSION: u32 = 1;

/// Maximum number of admins. The admin list lives in instance storage, which
/// is loaded on every invocation, so it must stay small.
pub const MAX_ADMINS: u32 = 10;

/// Upper bound for the Folder claim fee, in the fee token's smallest unit
/// (10_000 XLM for a 7-decimal token).
pub const MAX_CLAIM_FEE: i128 = 10_000 * 10_000_000;

/// Maximum amount a single `withdraw` call may move out of the treasury, in
/// the token's smallest unit (100_000 XLM for a 7-decimal token). Larger
/// withdrawals must be split across calls, each emitting its own event.
pub const MAX_WITHDRAW_PER_CALL: i128 = 100_000 * 10_000_000;

/// Delay applied to queued admin changes (~24h at ~5 seconds per ledger).
pub const ADMIN_TIMELOCK_LEDGERS: u32 = DAY_IN_LEDGERS;

/// Maximum number of admin changes that may be queued at once. Bounds the
/// size of the pending queue stored in instance storage.
pub const MAX_PENDING_ADMIN_CHANGES: u32 = 16;

/// Maximum number of length-based price tiers.
pub const MAX_PRICE_TIERS: u32 = 5;
/// Seconds per day, used to express lease periods in days.
pub const SECONDS_PER_DAY: u64 = 86_400;

/// Minimum number of ledgers between `commit_claim` and `reveal_claim`
/// (~50s). An observer who learns the name from a pending reveal can only
/// commit it in that ledger, so they cannot reveal before the honest reveal
/// lands. The margin covers a reveal delayed by congestion.
pub const MIN_REVEAL_DELAY: u32 = 10;
/// Number of ledgers after `commit_claim` during which the commit can be
/// revealed (~1 day). Older commits are treated as missing.
pub const REVEAL_WINDOW: u32 = DAY_IN_LEDGERS;
/// TTL of a claim commit in temporary storage: just above the reveal window,
/// so the entry outlives every ledger in which it can still be revealed.
pub const CLAIM_COMMIT_TTL: u32 = REVEAL_WINDOW + 1;

/// Maximum number of recovery guardians per owner.
pub const MAX_GUARDIANS: u32 = 5;
/// Ledgers between `initiate_recovery` and the earliest `execute_recovery`
/// (~3 days): the window in which the owner can `cancel_recovery`.
pub const RECOVERY_DELAY: u32 = 3 * DAY_IN_LEDGERS;

/// `revoke_folder` reason codes (documented in the README). New codes are
/// appended, never renumbered.
pub const REVOKE_REASON_IMPERSONATION: u32 = 1;
pub const REVOKE_REASON_ABUSE: u32 = 2;
pub const REVOKE_REASON_LEGAL: u32 = 3;
/// Highest accepted reason code.
pub const MAX_REVOKE_REASON: u32 = REVOKE_REASON_LEGAL;
