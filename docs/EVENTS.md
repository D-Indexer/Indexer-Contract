# Contract Events

Events are typed Soroban `#[contractevent]` structs in [`src/events.rs`](../src/events.rs). The
backend indexer (`Indexer-Backend`) depends on this file. Update it in the same PR as any change
to `src/events.rs`.

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
| `adm_add` | `AdminAdded` | `admin: Address` (caller), `new_admin: Address` | `add_admin` |
| `adm_rm` | `AdminRemoved` | `admin: Address` (caller), `target: Address` | `remove_admin` |
| `folder_cl` | `FolderClaimed` | `owner: Address`, `folder_name: String` | `claim_folder` |
| `tmpl_reg` | `TemplateRegistered` | `template_id: u32`, `metadata_uri: String`, `creator: Address` | `register_template` |
| `tmpl_set` | `TemplateAssigned` | `owner: Address`, `template_id: u32` | `set_user_template` |
| `port_mint` | `PortfolioMinted` | `owner: Address`, `template_id: u32`, `ipfs_cid: String` | `mint_portfolio` |
| `port_upd` | `PortfolioUpdated` | `owner: Address`, `ipfs_cid: String` | `update_portfolio` |

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
| `folder_cl` | Upsert identity `owner ↔ folder_name` |
| `tmpl_reg` | Cache template `template_id → metadata_uri, creator` and fetch the metadata from IPFS |
| `tmpl_set` | Set the owner's template and the portfolio's `template_id` if one is minted |
| `port_mint` | Insert the portfolio (`owner`, `template_id`, `ipfs_cid`) and set the owner's template |
| `port_upd` | Update the portfolio's `ipfs_cid` |

Ledger sequence, close time and tx hash come from the RPC `getEvents` envelope, so they aren't
duplicated in event data. Use `ledger_closed_at` as the mint or update time. `minted_at` is also
stored on-chain and returned by `get_portfolio`.

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

These are tracked as GitHub issues:

- Events carry no schema version field.
- `owner` is in data rather than topics, so RPC topic filters can't select one owner's events.
