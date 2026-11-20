# ledger-keys

Prints the base64 XDR `LedgerKey` for a Folder contract storage entry, so the
backend can read storage with RPC `getLedgerEntries` (cheap, batchable)
instead of simulating getters. It reuses the contract's own `DataKey` type
(via the contract crate's `lib` feature), so keys can't drift from the
contract.

```bash
cargo run -q -p ledger-keys -- <contract-id> <entry> [arg]

cargo run -q -p ledger-keys -- CDDQK74SMH4RB4HYLHM6BYZPTAMDMRISHW5Q274KADVZVFD57K43O6KV template 1
cargo run -q -p ledger-keys -- <contract-id> folder-owner alice.folder
cargo run -q -p ledger-keys -- <contract-id> portfolio G...
```

| Entry | `DataKey` | Durability |
| --- | --- | --- |
| `instance` | contract instance: `AdminList`, `Initialized`, `Treasury` | persistent (instance entry) |
| `folder-owner <name>` | `FolderOwner(name)` | persistent |
| `owner-folder <address>` | `OwnerFolder(address)` | persistent |
| `template <id>` | `Template(id)` | persistent |
| `template-list` | `TemplateList` | persistent |
| `template-price <id>` | `TemplatePrice(id)` | persistent |
| `user-template <address>` | `UserTemplate(address)` | persistent |
| `portfolio <address>` | `Portfolio(address)` | persistent |

Instance-stored values have no entry of their own: read the `instance` key and
look them up in the returned instance's `storage` map.

## Using the output from JavaScript

```js
import { rpc, xdr, scValToNative } from "@stellar/stellar-sdk";

const server = new rpc.Server("https://soroban-testnet.stellar.org");
const keys = [
  "AAAABgAAAAHHBX+SYfkQ8PhZ2eDjL5gYNkUSPbsNf4oA65qUffq5twAAABAAAAABAAAAAgAAAA8AAAAIVGVtcGxhdGUAAAADAAAAAQAAAAE=", // template 1
].map((k) => xdr.LedgerKey.fromXDR(k, "base64"));

const { entries } = await server.getLedgerEntries(...keys);
for (const e of entries) {
  console.log(scValToNative(e.val.contractData().val()), "live until", e.liveUntilLedgerSeq);
}
```

Missing keys are simply absent from `entries` (never written, or archived).

## Tests

`cargo test -p ledger-keys` drives the contract in a testutils `Env` and checks
that every key the tool prints matches an entry the contract actually wrote.
