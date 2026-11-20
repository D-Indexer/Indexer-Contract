# Storage Model

`DataKey` is defined in [`src/types.rs`](../src/types.rs). All I/O goes through
[`src/storage.rs`](../src/storage.rs).

| Key | Storage | Value | Bound |
|---|---|---|---|
| `Initialized` | instance | `bool` | 1 |
| `AdminList` | instance | `Vec<Address>` | `MAX_ADMINS` = 10 |
| `Template(u32)` | persistent | `Template` | `MAX_TEMPLATES` = 256 |
| `TemplateCount` | persistent | `u32` | 1 |
| `TemplatePage(u32)` | persistent | `Vec<u32>` (≤ 32 IDs) | 8 pages |
| `TemplateList` (legacy) | persistent | `Vec<u32>` | removed on first registration |
| `FolderOwner(String)` | persistent | `Address` | one per owner |
| `OwnerFolder(Address)` | persistent | `String` | one per owner |
| `UserTemplate(Address)` | persistent | `u32` | one per owner |
| `Portfolio(Address)` | persistent | `PortfolioNFT` | one per owner |
| `PaymentAddress(String)` | persistent | `Address` | one per Folder |
| `FolderRecord(String, Symbol)` | persistent | `String` (≤ 256 bytes) | `MAX_RECORDS` = 16 per Folder |
| `FolderRecordKeys(String)` | persistent | `Vec<Symbol>` | `MAX_RECORDS` = 16 per Folder |

Instance storage is a single ledger entry that every invocation loads, so it holds only small,
bounded values. Templates used to live there and could have grown it without limit. They are now
in persistent storage.

## Template ID list

Template IDs are stored in registration order in pages of `TEMPLATE_PAGE_SIZE` = 32 IDs, with a
`TemplateCount` counter. Registering a template rewrites only the counter and the last page, so
the bytes written no longer grow with the registry. One entry per ID (`TemplateAt(index)`) was
rejected: `list_templates` would then read up to 257 entries, which exceeds the per-transaction
ledger entry limit. Pages keep a full listing at 9 reads.

A contract that still has the legacy `TemplateList` vector keeps reading it until the next
`register_template`, which moves it into pages and deletes it. The migration runs once.

Registration cost measured with `env.cost_estimate().resources()` (`test_template_registration_cost_is_constant`):

| Registry size | `write_bytes` before (single `Vec`) | `write_bytes` after (pages) |
|---|---|---|
| 1 | 464 | 572 |
| 32 | 712 | 820 |
| 128 | 1480 | 820 |
| 256 | 2504 | 820 |

## Folder records

Each record is its own entry, `FolderRecord(name, key)`, with a key index `FolderRecordKeys(name)`.
`get_record` reads one entry of at most 256 bytes. Overwriting a value writes only that entry. A
single `Map` would make every read and write load and rewrite all 16 records (up to about 4 KB).
`get_records` reads the index plus one entry per record (at most 17 reads).

Keys must match `[a-z0-9_]{1,16}`. Suggested keys: `website`, `github`, `twitter`, `email`,
`avatar`, `contact_cid`, `description`. Values are 1 to 256 bytes of printable ASCII (0x20–0x7E).

Records and the payment address are keyed by Folder name, not by owner address. If name transfer
is ever added, they move with the name, and the new owner should clear or overwrite them.

## Binary CIDs (evaluated, not adopted)

Issue #266 proposed storing CIDs as decoded bytes. Entry sizes measured by
`test_binary_cid_storage_savings` (XDR, owner address included):

| CID | String CID | Binary CID | Saved | Portfolio entry |
|---|---|---|---|---|
| CIDv0 (46 chars, base58btc) | 56 bytes | 44 bytes | 12 bytes | 200 bytes |
| CIDv1 raw (59 chars, base32) | 68 bytes | 44 bytes | 24 bytes | 212 bytes |

The saving is 6–11% of one entry per portfolio, paid once in rent and per update in write fees.
Doing the conversion on-chain would add a base58 decoder (big-number division, quadratic in
length) and a base32 codec to the WASM. That makes every mint, update and read cost more CPU and
larger code, and existing entries would need a migration. The CPU cost was not benchmarked
on-chain; the decision rests on the small byte saving. **Decision:** keep CIDs as strings. Events
keep carrying the string, which indexers can use directly.

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
