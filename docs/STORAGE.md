# Storage Model

`DataKey` is defined in [`src/types.rs`](../src/types.rs). All I/O goes through
[`src/storage.rs`](../src/storage.rs).

| Key | Storage | Value | Bound |
|---|---|---|---|
| `Initialized` | instance | `bool` | 1 |
| `AdminList` | instance | `Vec<Address>` | `MAX_ADMINS` = 10 |
| `Template(u32)` | persistent | `Template` | `MAX_TEMPLATES` = 256 |
| `TemplateList` | persistent | `Vec<u32>` | `MAX_TEMPLATES` = 256 |
| `FolderOwner(String)` | persistent | `Address` | one per owner |
| `OwnerFolder(Address)` | persistent | `String` | one per owner |
| `UserTemplate(Address)` | persistent | `u32` | one per owner |
| `Portfolio(Address)` | persistent | `PortfolioNFT` | one per owner |

Instance storage is a single ledger entry that every invocation loads, so it holds only small,
bounded values. Templates used to live there and could have grown it without limit. They are now
in persistent storage.

## TTL policy

Constants are in [`src/constants.rs`](../src/constants.rs). One day is about 17,280 ledgers.

| Storage | Extended to | Re-extended when TTL drops below | When |
|---|---|---|---|
| Instance + code | 120 days | 90 days | Every state-changing call (`initialize`, admin, claim, template, mint, update) |
| Persistent | 120 days | 90 days | Every read or write of that entry |

- 120 days is below the testnet and mainnet `max_entry_ttl` of about 180 days (3,110,400
  ledgers), so the host never clamps the request. A test pins this.
- The 30-day gap between the two values means each entry pays for an extension at most about
  once a month.
- An entry that nobody touches for about 120 days is archived. It can be restored with
  `stellar contract restore`; it is not lost.
- Read-only calls that are only simulated, never submitted, don't persist extensions.

## Public TTL extension

Two unauthenticated entry points let a backend or cron job keep active records alive without
touching their data:

| Function | Keys extended | Returns |
|---|---|---|
| `extend_owner_ttl(owner)` | `OwnerFolder(owner)`, `FolderOwner(name)`, `UserTemplate(owner)`, `Portfolio(owner)` | number of keys extended |
| `extend_template_ttl(template_id)` | `Template(template_id)`, `TemplateList` | number of keys extended |

- Neither function requires a signature and neither writes or changes any value.
- Missing keys are skipped silently, so calling them for an owner or template that does not
  exist is a no-op that returns `0`.
- Each key is extended to 120 days, subject to the same threshold as every other read: a key
  whose remaining TTL is still above 90 days is not re-extended, so repeated calls are cheap.

### Suggested schedule

- Run the job **once a day** for every owner seen in the last 30 days (e.g. owners with recent
  `claim_folder`, `set_user_template`, `mint_portfolio` or `update_portfolio` activity).
- Run it **once a week** for every registered template ID (`list_templates`).
- Because of the 90-day threshold, a daily run only pays rent for entries that actually need it;
  a monthly run is the minimum that keeps active records from ever reaching the 120-day archive
  point. Prefer daily so a missed run or two is harmless.
- Batch calls (a few dozen per transaction) to stay well inside the per-transaction resource
  limits; the functions are idempotent, so retries are safe.
