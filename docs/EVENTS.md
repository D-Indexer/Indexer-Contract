# Contract Events

Events are typed Soroban `#[contractevent]` structs in [`src/events.rs`](../src/events.rs). The
backend indexer (`Indexer-Backend`) depends on this file. Update it in the same PR as any change
to `src/events.rs`.

The machine-readable schema is [`events.schema.json`](../events.schema.json), generated from
`src/events.rs` by [`scripts/events_schema.py`](../scripts/events_schema.py). Regenerate it with
`make events-schema`; `make events-schema-check` (part of `make check` and CI) fails when the
committed file is stale. Do not edit the schema by hand.

## Encoding

Every event has:

- **topics**: a single `Symbol`, the short name below (`topics[0]`). No data fields are in topics.
- **data**: an `ScVal::Vec` holding the fields **in the order listed** (`data_format = "vec"`).

Compatibility rules:

- Fields are only ever **appended** to the end of an event's data vec. Decode positionally and
  ignore trailing elements you don't know about.
- Topic symbols are never renamed. A breaking change gets a new topic instead.

## Events

| Topic | Struct | Data (in order) | Emitted by |
|---|---|---|---|
| `init` | `ContractInitialized` | `admin: Address` | `initialize` |
| `adm_add` | `AdminAdded` | `admin: Address` (caller), `new_admin: Address` | `execute_admin_change` (`Add`) |
| `adm_rm` | `AdminRemoved` | `admin: Address` (caller), `target: Address` | `execute_admin_change` (`Remove`) |
| `adm_q` | `AdminChangeQueued` | `admin: Address`, `op_id: u32`, `target: Address`, `eta_ledger: u32` | `queue_admin_change` |
| `adm_ex` | `AdminChangeExecuted` | `admin: Address`, `op_id: u32`, `target: Address` | `execute_admin_change` |
| `adm_cx` | `AdminChangeCancelled` | `admin: Address`, `op_id: u32`, `target: Address` | `cancel_admin_change` |
| `upgraded` | `ContractUpgraded` | `admin: Address`, `new_wasm_hash: BytesN<32>` | `upgrade` |
| `folder_cl` | `FolderClaimed` | `owner: Address`, `folder_name: String`, `fee_paid: i128` | `claim_folder` |
| `tmpl_reg` | `TemplateRegistered` | `template_id: u32`, `metadata_uri: String`, `creator: Address` | `register_template` |
| `tmpl_set` | `TemplateAssigned` | `owner: Address`, `template_id: u32` | `set_user_template` |
| `port_mint` | `PortfolioMinted` | `owner: Address`, `template_id: u32`, `ipfs_cid: String` | `mint_portfolio`, `claim_and_mint` |
| `port_upd` | `PortfolioUpdated` | `owner: Address`, `ipfs_cid: String` | `update_portfolio` |
| `fee_set` | `ClaimFeeSet` | `admin: Address`, `token: Address`, `amount: i128`, `treasury: Address` | `set_claim_fee` |
| `withdraw` | `Withdrawn` | `admin: Address`, `token: Address`, `to: Address`, `amount: i128` | `withdraw` |
| `blk_add` | `AddressBlocked` | `admin: Address`, `address: Address`, `reason_code: u32` | `block_address` |
| `blk_rm` | `AddressUnblocked` | `admin: Address`, `address: Address` | `unblock_address` |
| `port_flag` | `PortfolioFlagSet` | `admin: Address`, `owner: Address`, `flag: u32`, `reason_code: u32` | `set_portfolio_flag` |
| `tmpl_sub` | `TemplateSubmitted` | `submission_id: u32`, `designer: Address`, `metadata_uri: String` | `submit_template` |
| `tmpl_appr` | `TemplateSubmissionApproved` | `admin: Address`, `submission_id: u32`, `template_id: u32` | `approve_submission` |
| `tmpl_rej` | `TemplateSubmissionRejected` | `admin: Address`, `submission_id: u32`, `reason_code: u32` | `reject_submission` |

### `claim_and_mint` (#157)

- `claim_and_mint` emits the same two events as `claim_folder` followed by `mint_portfolio`, in
  that order and in the same transaction: `folder_cl`, then `port_mint`. Indexers need no new
  handling; apply them in event order.

### Changes in the economics / soroban-sdk 29 update

- **New:** `fee_set`, `withdraw`.
- **Appended:** `fee_paid` as the third field of `folder_cl` (0 when no fee is configured).
  Decoders that read the first two fields keep working.
- **soroban-sdk 25.3.1 → 29.0.1:** on-chain event XDR encoding is unchanged apart from the
  appended field above. Test snapshots now record protocol 29 and spell the event type key
  `type` instead of `type_` (snapshot JSON only). Run `make sdk-compat` for the full report.

### Changes in `feat/testnet-deploy`

- **New:** `init`, `adm_add`, `adm_rm`. Before this, admin changes emitted nothing.
- **Appended:** `creator` as the third field of `tmpl_reg`. Decoders that read the first two
  fields keep working.
- **Semantics:**
  - `mint_portfolio` now also sets the owner's selected template. It emits only `port_mint`, so
    treat `port_mint.template_id` as the owner's current template.
  - `set_user_template` now also changes `template_id` on an already-minted portfolio. On
    `tmpl_set`, update the owner's portfolio record as well as the selection.

## Indexer handling

| Topic | Suggested backend action |
|---|---|
| `init` | Seed the admin set with `admin` |
| `adm_add` / `adm_rm` | Add / remove the admin from the admin set |
| `folder_cl` | Upsert identity `owner ↔ folder_name` (a re-claim of an expired name replaces the previous owner) |
| `claim_spon` | Record the sponsor of the claim |
| `folder_rn` | Update the Folder's lease expiry |
| `rec_init` / `rec_appr` / `rec_cancel` | Track the pending recovery (for owner and guardian notifications) |
| `rec_exec` | Re-key the identity: move the Folder name, portfolio and template selection from `owner` to `new_owner` and drop every record of `owner` |
| `tmpl_reg` | Cache template `template_id → metadata_uri, creator` and fetch the metadata from IPFS |
| `tmpl_set` | Set the owner's template and the portfolio's `template_id` if one is minted |
| `port_mint` | Insert the portfolio (`owner`, `template_id`, `ipfs_cid`) and set the owner's template |
| `port_upd` | Update the portfolio's `ipfs_cid`; use `previous_cid` to detect gaps (see below) |

### Portfolio moderation flags

`PortfolioFlag(owner)` is the authoritative on-chain moderation signal:

- `0` means no flag. The key is removed.
- `1` means hidden. Frontends should hide the portfolio content and do not delete its record.
- `2` means under review. Frontends should keep it visible only when their review policy allows
  it, and backends should retain the portfolio row for audit and recovery.

The backend should consume `port_flag` by setting `get_portfolio_flag(owner)` semantics:
`(admin, owner, flag, reason_code)`. An owner `update_portfolio` must not clear an existing flag.

Ledger sequence, close time and tx hash come from the RPC `getEvents` envelope, so they aren't
duplicated in event data. Use `ledger_closed_at` as the mint or update time. `minted_at` is also
stored on-chain and returned by `get_portfolio`.

## Gap detection for `port_upd`

`port_upd` now carries `previous_cid` as its last data field: the CID that was stored on-chain
immediately before this update. Positional decoders that read only `owner` and `ipfs_cid` keep
working; the new field is appended, never inserted.

Use it to detect missed events:

- Keep the last CID you indexed per owner (from `port_mint.ipfs_cid` or the previous
  `port_upd.ipfs_cid`).
- On each `port_upd`, compare the stored CID with `previous_cid`:
  - **equal** → the chain is contiguous; apply `ipfs_cid` and store it.
  - **different** → at least one `port_upd` was missed (or the record is stale). Do not trust the
    incremental state: re-read the portfolio with `get_portfolio(owner)` and re-sync, then log the
    gap for the owner.
- `previous_cid` is the CID before the write, so a self-update (same CID) yields
  `previous_cid == ipfs_cid`; treat that as a no-op, not a gap.
- Events are ordered per ledger; if several `port_upd` events for one owner land in the same
  ledger, apply them in event order and check each `previous_cid` against the running value.

## Example (testnet)

Output of `stellar events --id <CONTRACT_ID> --network testnet --output json` for the deployment
in [`deployments/testnet.json`](../deployments/testnet.json):

```json
{"ledger":5647760,"event_name":"ContractInitialized","prefix_topics":["init"],"params":{"admin":"GCFADTAQUXYFXUW7G65YXIQTTDORMT72FFTUYFEEUXJZDNAYTD2RXMBZ"}}
{"ledger":5647768,"event_name":"TemplateRegistered","prefix_topics":["tmpl_reg"],"params":{"template_id":1,"metadata_uri":"ipfs://bafkreiejxg6d3gqvi3737mcwxkv4u7gismaraxgv5zfoqtfuc3nbdgve7i","creator":"GCFADTAQUXYFXUW7G65YXIQTTDORMT72FFTUYFEEUXJZDNAYTD2RXMBZ"}}
{"ledger":5647861,"event_name":"FolderClaimed","prefix_topics":["folder_cl"],"params":{"owner":"GAPAMTTVHB7RZ5DQHEINZN3GQYRCXXWNCQNQSY5YAU6RKQUYX2QJT2CM","folder_name":"smoke-1794262900.folder"}}
{"ledger":5647869,"event_name":"PortfolioMinted","prefix_topics":["port_mint"],"params":{"owner":"GAPAMTTVHB7RZ5DQHEINZN3GQYRCXXWNCQNQSY5YAU6RKQUYX2QJT2CM","template_id":1,"ipfs_cid":"bafkreie33ewzcjvpo3gldpvqgevtoy4oaygkqri3cwxqr6o6lsqc4jqvwu"}}
{"ledger":5647878,"event_name":"PortfolioUpdated","prefix_topics":["port_upd"],"params":{"owner":"GAPAMTTVHB7RZ5DQHEINZN3GQYRCXXWNCQNQSY5YAU6RKQUYX2QJT2CM","ipfs_cid":"bafkreifwfa7v77kn2om36djjtqiwqykldpady3ytsfbqkqgcrfilhjqtnu"}}
```

The CLI shows `params` with field names decoded from the contract spec. On the wire, the data is
the positional vec described above.

## Known limitations

- Events carry no schema version field ([#137](https://github.com/D-Indexer/Indexer-Contract/issues/137)).
- `owner` is in data rather than topics, so RPC topic filters can't select one owner's events.
  Filter by contract ID and route on `topics[0]` instead.
