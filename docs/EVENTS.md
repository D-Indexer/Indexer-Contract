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
| `init` | `ContractInitialized` | `admin: Address`, `seq: u64` | `initialize` |
| `adm_add` | `AdminAdded` | `admin: Address` (caller), `new_admin: Address`, `seq: u64` | `add_admin` |
| `adm_rm` | `AdminRemoved` | `admin: Address` (caller), `target: Address`, `seq: u64` | `remove_admin` |
| `folder_cl` | `FolderClaimed` | `owner: Address`, `folder_name: String`, `seq: u64` | `claim_folder` |
| `tmpl_reg` | `TemplateRegistered` | `template_id: u32`, `metadata_uri: String`, `creator: Address`, `seq: u64` | `register_template` |
| `tmpl_set` | `TemplateAssigned` | `owner: Address`, `template_id: u32`, `seq: u64` | `set_user_template` |
| `port_mint` | `PortfolioMinted` | `owner: Address`, `template_id: u32`, `ipfs_cid: String`, `slot: u32`, `seq: u64` | `mint_portfolio`, `mint_portfolio_v2`, `mint_portfolio_slot` |
| `port_upd` | `PortfolioUpdated` | `owner: Address`, `ipfs_cid: String`, `slot: u32`, `seq: u64` | `update_portfolio`, `update_portfolio_v2`, `update_portfolio_slot` |

`slot` is the portfolio slot: `0` for the legacy single portfolio (and the legacy entry points),
otherwise the slot passed to `mint_portfolio_slot` / `update_portfolio_slot`.

## Sequence and digest

Every event ends with `seq: u64`, a contract-wide counter that starts at `1` and increases by
exactly one per emitted event. Failed calls roll back and consume no number, so a gap in the
indexed sequence means a missed event.

The contract also keeps a rolling digest over every event. Starting from 32 zero bytes:

```
digest_n = sha256(digest_{n-1} || seq_n || topics_xdr || data_xdr)
```

- `digest_{n-1}`: the previous 32-byte digest (`[0u8; 32]` before the first event).
- `seq_n`: the event's `seq`, 8 bytes, big-endian `u64`.
- `topics_xdr`: XDR of the topics as one `ScVal::Vec`, e.g. `[Symbol("init")]` encodes as
  `00000010 00000001 00000001 0000000f 00000004 696e6974`.
- `data_xdr`: XDR of the event data `ScVal::Vec` exactly as emitted, `seq` included.

`get_event_cursor() -> (u64, BytesN<32>)` returns the last `seq` and `digest` (`(0, zeros)` before
any event). An indexer that recomputes the same digest from the events it ingested has processed
every event up to that `seq`.

Cost: the sha256 over a typical ~260-byte preimage is ~19k CPU instructions in the test host,
plus two small instance-storage writes, against ~440k for a whole `mint_portfolio` and ~330k for
`update_portfolio`.

### Changes for issues #250 and #252

- **Appended:** `seq` as the last field of every event; `slot` before `seq` on `port_mint` and
  `port_upd`.

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

- Events carry no schema version field ([#137](https://github.com/D-Indexer/Indexer-Contract/issues/137)).
- `owner` is in data rather than topics, so RPC topic filters can't select one owner's events.
  Filter by contract ID and route on `topics[0]` instead.
