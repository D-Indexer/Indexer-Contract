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
| `PortfolioHistory(Address)` | persistent | `Vec<CidEntry { cid, at }>`, oldest first | `MAX_CID_HISTORY` = 10 entries per owner |

Instance storage is a single ledger entry that every invocation loads, so it holds only small,
bounded values. Templates used to live there and could have grown it without limit. They are now
in persistent storage.

## Portfolio CID history

`PortfolioHistory(owner)` keeps the owner's last `MAX_CID_HISTORY` (10) portfolio CIDs, each with
the ledger timestamp (`at`) at which it became the content. It is a ring buffer: `mint_portfolio`,
`claim_and_mint` and every successful `update_portfolio` append one entry, and once 10 are stored
the oldest is dropped. The newest entry is always the current `Portfolio.ipfs_cid`. Rejected
calls append nothing. Read it with `get_portfolio_history(owner)`; owners who never minted get an
empty vector.

The entry follows the persistent TTL policy below like every other per-owner key: it is extended
to 120 days on every write and on every `get_portfolio_history` read once below 90 days.

Write cost, measured by the `cid_history::full_history_write_cost` test (soroban-sdk 29 cost
estimate, 59-character CIDv1 values):

| | Bytes |
|---|---|
| Full 10-entry history value (XDR) | 1,172 (about 117 per entry) |
| `update_portfolio` writes, history holding 2 entries | 788 |
| `update_portfolio` writes, history full | 1,716 |

The number of written entries doesn't change once the history is full; only the size does. With
the longest accepted CID (128 characters) a full history is roughly 1.9 KB, well under the
network's per-entry size limit.

**Burn policy.** The contract has no burn or portfolio-removal entry point. If one is added, it
must delete `PortfolioHistory(owner)` together with `Portfolio(owner)`, so a later mint by the same
owner starts a fresh history. Until then, history entries only leave through the ring buffer or
by archival after 120 untouched days.

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

## Upgrades

`upgrade(admin, new_wasm_hash)` is admin-only and calls
`env.deployer().update_current_contract_wasm`. It replaces the contract **code** only:

- Instance storage (`Initialized`, `AdminList`) and every persistent entry (`Template`,
  `TemplateList`, `FolderOwner`, `OwnerFolder`, `UserTemplate`, `Portfolio`) are preserved
  unchanged. No migration step is needed for a code-only upgrade.
- The admin list is read from storage, so the same admins keep control after the upgrade.
- `version()` returns `CONTRACT_VERSION` from [`src/constants.rs`](../src/constants.rs). Bump it
  in the same PR as any upgrade so operators can confirm which code is live.

Compatibility rules for upgrades:

- **Never** rename or repurpose a `DataKey` variant, and never change the type of an existing
  value. Add new variants instead; old entries stay readable.
- New keys must tolerate being absent (`Option`/default) so a contract upgraded from an older
  version keeps working without a migration.
- Removing a key is only safe once no live deployment can still hold it.
- If a layout change is unavoidable, ship a migration entry point in the new code and run it
  right after `upgrade` in the same release.

Use [`scripts/upgrade.sh`](../scripts/upgrade.sh) to upload and apply a new wasm hash.

