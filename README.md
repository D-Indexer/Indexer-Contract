# Indexer-Contract

[![Soroban Contract CI](https://github.com/D-Indexer/Indexer-Contract/actions/workflows/contract-ci.yml/badge.svg)](https://github.com/D-Indexer/Indexer-Contract/actions/workflows/contract-ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

The Soroban smart contract (Rust, `soroban-sdk` 25.3.1) behind **Folder**, a decentralized
portfolio tool on Stellar. A user claims a unique `name.folder` handle tied to their Stellar
address, picks one of the admin-registered portfolio templates, and anchors their portfolio on-chain
as a record that links owner, template and the IPFS CID of the portfolio content. Anyone can look up
a Folder name or verify that an address holds a portfolio. Every state change emits a typed event
that the Folder backend indexes. The frontend
([`Indexer-Frontend`](https://github.com/D-Indexer/Indexer-Frontend)) and backend
([`Indexer-Backend`](https://github.com/D-Indexer/Indexer-Backend)) live in separate repositories.

## Testnet deployment

| | |
|---|---|
| Contract ID | [`CDDQK74SMH4RB4HYLHM6BYZPTAMDMRISHW5Q274KADVZVFD57K43O6KV`](https://stellar.expert/explorer/testnet/contract/CDDQK74SMH4RB4HYLHM6BYZPTAMDMRISHW5Q274KADVZVFD57K43O6KV) |
| Wasm hash | `83fa464a26a7827a4b2a2b8d23f5a28d8c62a0536c0f373f42c76631ade2396b` |
| Network / RPC | `testnet` / `https://soroban-testnet.stellar.org` |
| Admin | [`GCFADTAQ…TD2RXMBZ`](https://stellar.expert/explorer/testnet/account/GCFADTAQUXYFXUW7G65YXIQTTDORMT72FFTUYFEEUXJZDNAYTD2RXMBZ) |
| Templates | `1`, `2`, `3` ([`templates/`](templates/)) |
| Deployed | 2026-11-09 |

The full record, including transaction hashes, is in
[`deployments/testnet.json`](deployments/testnet.json). The wasm hash can be reproduced with
`make wasm-hash` at the current commit. An end-to-end run against this contract (claim → mint →
update → verify, with tx links) is in [`docs/testnet-smoke-run.md`](docs/testnet-smoke-run.md).

## Status

| Area | Status | Notes |
|---|---|---|
| Folder name claim and lookup (both directions) | ✅ | One name per address, one address per name |
| Template registry (register, get, list) | ✅ | Admin-only, at most 256 templates, metadata must be `ipfs://` |
| Template selection per user | ✅ | Kept in sync with the minted portfolio |
| Portfolio mint / update / read | ✅ | One portfolio per address |
| `verify_portfolio` | 🟡 | Returns whether the address has a portfolio. It does not check the content behind the CID. |
| Input validation (names, CIDs, URIs, IDs) | ✅ | Checks shape only, see [docs/VALIDATION.md](docs/VALIDATION.md) |
| Admin management | 🟡 | Add/remove with a 10-admin cap and last-admin protection. Any single admin can act alone; no two-step transfer or threshold. |
| Events for every state change | 🟡 | All writes emit events. There is no schema version field yet. |
| Storage TTL extension | ✅ | Instance and persistent entries extended to 120 days, see [docs/STORAGE.md](docs/STORAGE.md) |
| Testnet deployment + smoke test | ✅ | Scripted, recorded, and re-runnable from CI |
| CI (fmt, clippy, tests, optimized wasm) | ✅ | On every PR; the wasm is uploaded as an artifact |
| Initialization in the deploy transaction (constructor) | ⬜ | `initialize` is a separate call; the deploy script verifies the admin afterwards |
| Template versioning / deprecation | ⬜ | Registered templates can't be changed or retired |
| Folder name release or transfer | ⬜ | Names are permanent |
| Portfolio transfer | ⬜ | Portfolios are bound to the minting address; this is not a transferable token standard |
| Contract upgrade entry point | ⬜ | A new version means a new contract ID |
| Fuzz / property tests | ⬜ | Validation is covered by example-based unit tests only |
| Mainnet deployment | ⬜ | See [docs/RELEASE_CHECKLIST.md](docs/RELEASE_CHECKLIST.md) |

## Quick start

Prerequisites: [rustup](https://rustup.rs) (the toolchain pinned in
[`rust-toolchain.toml`](rust-toolchain.toml), Rust 1.99.1 with `wasm32v1-none`, installs on first
use), [Stellar CLI](https://github.com/stellar/stellar-cli/releases) 28.1.0, `jq`, `python3`.

```bash
# Build
make build-optimized      # stellar contract build --optimize -> target/stellar/indexer_contract.wasm
make wasm-hash            # sha256 = the on-chain wasm hash

# Test
make test                 # cargo test (61 unit tests, Soroban testutils)
make check                # fmt-check, cargo check, clippy -D warnings, tests, wasm build, script syntax

# Deploy to testnet (creates and Friendbot-funds the `folder-deployer` identity if needed,
# initializes, registers templates/manifest.json, writes deployments/testnet.json)
./scripts/deploy.sh testnet

# End-to-end check against deployments/testnet.json
./scripts/smoke-testnet.sh
```

[docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) covers identities, overrides and mainnet.

Example calls against the testnet contract:

```bash
ID=CDDQK74SMH4RB4HYLHM6BYZPTAMDMRISHW5Q274KADVZVFD57K43O6KV
stellar contract invoke --id $ID --network testnet --source-account me -- \
  claim_folder --owner me --folder_name alice.folder
stellar contract invoke --id $ID --network testnet --source-account me -- \
  mint_portfolio --owner me --template_id 1 \
  --ipfs_cid bafkreiejxg6d3gqvi3737mcwxkv4u7gismaraxgv5zfoqtfuc3nbdgve7i
stellar contract invoke --id $ID --network testnet --source-account me --send=no -- \
  get_portfolio --owner me
```

## Public functions

"Signer" is the argument that must authorize the call (`require_auth`). Every write except
`initialize` first fails with `NotInitialized` (2) if the contract hasn't been initialized, and
extends the instance TTL. Reads have no signer and emit nothing.

### Writes

| Function | Arguments | Signer | Event | Errors |
|---|---|---|---|---|
| `initialize` | `admin: Address` | `admin` | `init` | `AlreadyInitialized` (1) |
| `add_admin` | `admin: Address, new_admin: Address` | `admin` (must be an admin) | `adm_add` | 2, `Unauthorized` (7), `AdminAlreadyExists` (10), `TooManyAdmins` (19) |
| `remove_admin` | `admin: Address, target: Address` | `admin` (must be an admin) | `adm_rm` | 2, 7, `AdminNotFound` (11), `CannotRemoveLastAdmin` (12) |
| `claim_folder` | `owner: Address, folder_name: String` | `owner` | `folder_cl` | 2, `InvalidFolderName` (9), `FolderAlreadyClaimed` (16), `FolderNameTaken` (3) |
| `register_template` | `admin: Address, template_id: u32, metadata_uri: String` | `admin` (must be an admin) | `tmpl_reg` | 2, 7, `InvalidTemplateID` (17), `InvalidMetadataURI` (14), `TemplateAlreadyExists` (15), `TemplateLimitReached` (18) |
| `set_user_template` | `owner: Address, template_id: u32` | `owner` | `tmpl_set` | 2, `FolderNotFound` (4), 17, `TemplateNotFound` (6) |
| `mint_portfolio` | `owner: Address, template_id: u32, ipfs_cid: String` | `owner` | `port_mint` | 2, 4, `PortfolioAlreadyMinted` (13), 17, `InvalidCID` (8), 6 |
| `update_portfolio` | `owner: Address, ipfs_cid: String` | `owner` | `port_upd` | 2, 8, `PortfolioNotFound` (5) |

Notes:

- `mint_portfolio` requires a claimed Folder name and also sets the owner's selected template.
- `set_user_template` also updates `template_id` on an existing portfolio.
- An admin may remove itself as long as another admin remains.

### Reads

| Function | Arguments | Returns | Errors |
|---|---|---|---|
| `get_admins` | — | `Vec<Address>` (empty before `initialize`) | — |
| `get_folder` | `folder_name: String` | owner `Address` | `FolderNotFound` (4) |
| `get_identity` | `owner: Address` | Folder name `String` | `FolderNotFound` (4) |
| `get_template` | `template_id: u32` | `Template { template_id, metadata_uri, creator }` | `InvalidTemplateID` (17), `TemplateNotFound` (6) |
| `list_templates` | — | `Vec<u32>` of registered IDs, in registration order | — |
| `get_user_template` | `owner: Address` | `Option<u32>` | — |
| `get_portfolio` | `owner: Address` | `PortfolioNFT { owner, template_id, ipfs_cid, minted_at }` | `PortfolioNotFound` (5) |
| `verify_portfolio` | `owner: Address` | `bool`: whether the owner has a portfolio | — |

### Error codes

Codes are stable. New errors are appended and never renumbered.

| Code | Error | Code | Error |
|---|---|---|---|
| 1 | `AlreadyInitialized` | 11 | `AdminNotFound` |
| 2 | `NotInitialized` | 12 | `CannotRemoveLastAdmin` |
| 3 | `FolderNameTaken` | 13 | `PortfolioAlreadyMinted` |
| 4 | `FolderNotFound` | 14 | `InvalidMetadataURI` |
| 5 | `PortfolioNotFound` | 15 | `TemplateAlreadyExists` |
| 6 | `TemplateNotFound` | 16 | `FolderAlreadyClaimed` |
| 7 | `Unauthorized` | 17 | `InvalidTemplateID` |
| 8 | `InvalidCID` | 18 | `TemplateLimitReached` |
| 9 | `InvalidFolderName` | 19 | `TooManyAdmins` |
| 10 | `AdminAlreadyExists` | | |

## How the backend indexer consumes events

The contract keeps no history or enumerable lists of users. The backend builds its queryable view
(owners, Folder names, portfolios, templates, admins) from contract events:

1. **Fetch.** Poll Soroban RPC `getEvents` with a `contractIds` filter set to the contract ID and
   page through with the returned cursor. Persist the cursor, because RPC only keeps events for a
   limited window of recent ledgers.
2. **Route.** `topics[0]` is a `Symbol` naming the event: `init`, `adm_add`, `adm_rm`,
   `folder_cl`, `tmpl_reg`, `tmpl_set`, `port_mint`, `port_upd`.
3. **Decode.** The event `value` is an `ScVal::Vec` with the fields in a fixed order (for example,
   `port_mint` is `[owner, template_id, ipfs_cid]`). Decode by position and ignore any trailing
   fields you don't know about: fields are only ever appended.
4. **Apply.** Upsert by owner address: `folder_cl` → identity, `port_mint`/`port_upd` → portfolio
   CID, `tmpl_set` → selected template *and* the portfolio's `template_id`, `tmpl_reg` → template
   cache plus an IPFS fetch of `metadata_uri`. Timestamps and tx hashes come from the `getEvents`
   envelope (`ledger`, `ledgerClosedAt`, `txHash`).
5. **Reconcile.** Use the read functions (`get_portfolio`, `get_identity`, `get_template`) to
   backfill or check individual records.

[docs/EVENTS.md](docs/EVENTS.md) is the contract between this repo and the backend: the field
order of every event, compatibility rules, recommended handling and real testnet output. Any PR
that changes `src/events.rs` must update it.

## Repository layout

| Path | Contents |
|---|---|
| `src/lib.rs` | Public contract interface; delegates to the modules below |
| `src/identity_map.rs`, `template_registry.rs`, `nft_mint.rs` | Folder names, templates, portfolios |
| `src/access_control.rs` | Admin and owner auth checks |
| `src/storage.rs`, `src/constants.rs` | All ledger I/O, TTL policy, limits |
| `src/validation.rs` | Folder name / CID / metadata URI / template ID rules |
| `src/events.rs`, `src/errors.rs`, `src/types.rs` | Events, error codes, stored types |
| `src/test.rs`, `test_snapshots/` | Unit tests and tracked auth snapshots |
| `scripts/` | `deploy.sh`, `smoke-testnet.sh`, `template_cids.py` |
| `templates/` | Template metadata JSON and its CID manifest |
| `deployments/` | Deployment records per network |

## Documentation

- [docs/EVENTS.md](docs/EVENTS.md): event schema (the backend depends on it)
- [docs/STORAGE.md](docs/STORAGE.md): storage keys, bounds and TTL policy
- [docs/VALIDATION.md](docs/VALIDATION.md): input rules with accepted and rejected examples
- [docs/SECURITY.md](docs/SECURITY.md): auth matrix, invariants, known risks
- [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md): deploying and redeploying
- [docs/RELEASE_CHECKLIST.md](docs/RELEASE_CHECKLIST.md): pre-release gates
- [docs/testnet-smoke-run.md](docs/testnet-smoke-run.md): recorded end-to-end testnet run

## Contributing

1. Pick an issue labelled `good first issue`, `medium` or `hard` and comment that you're taking it.
2. Branch from `main` and run `make check` before you push. CI runs the same gates.
3. If you change a public function, event or error, update this README and
   [docs/EVENTS.md](docs/EVENTS.md) in the same PR. Never renumber errors or reorder event fields.
4. Add a test for every behaviour change. Auth changes should update `test_snapshots/`.

## License

[MIT](LICENSE)
