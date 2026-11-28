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
| `Treasury` | instance | `Address` | 1 |
| `TemplatePrice(u32)` | persistent | `TemplatePrice` | `MAX_TEMPLATES` = 256 |

`tools/ledger-keys` prints the RPC `LedgerKey` for each of these.

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

