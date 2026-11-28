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
| `Portfolio(Address)` | persistent | `PortfolioNFT` | one per owner (slot 0) |
| `PortfolioSlot(Address, u32)` | persistent | `PortfolioNFT` | `MAX_PORTFOLIOS_PER_OWNER` - 1 per owner |
| `PortfolioSlots(Address)` | persistent | `Vec<u32>` (non-zero slots) | `MAX_PORTFOLIOS_PER_OWNER` - 1 = 4 |
| `PortfolioSchema(Address)` | persistent | `u32` (absent means 1) | one per owner |
| `TemplateSchemas(u32)` | persistent | `Vec<u32>` (absent means `[1]`) | `MAX_TEMPLATE_SCHEMAS` = 8 |
| `TemplateGated(u32)` | persistent | `bool` (absent means ungated) | one per template |
| `TemplateAllow(u32, Address)` | persistent | `()` | one per template and owner |
| `EventSeq` | instance | `u64` | 1 |
| `EventDigest` | instance | `BytesN<32>` | 1 |

Notes:

- **Slots:** slot 0 is the legacy `Portfolio(Address)` key, so the single-portfolio API and data
  are unchanged. Other slots are any `u32`; an owner can hold at most `MAX_PORTFOLIOS_PER_OWNER`
  = 5 portfolios, slot 0 included. `list_portfolio_slots` returns 0 first (if minted), then the
  other slots in mint order. `set_user_template` only changes the slot-0 portfolio.
- **Content schemas:** `mint_portfolio` and `update_portfolio` use schema 1. Mint, update and
  `set_user_template` fail with `IncompatibleSchema` when the template doesn't list the
  portfolio's schema. Extra slots always use schema 1.
- **Gated templates:** admins gate a template and manage its allowlist. `set_user_template` and
  every mint entry point reject non-allowlisted owners with `TemplateNotAllowed`. Removing an
  owner from the allowlist doesn't change a selection or portfolio they already have; it only
  blocks selecting or minting with that template again.

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

