# Deployment

## Prerequisites

- Rust 1.99.1 with the `wasm32v1-none` target. Both are installed automatically from
  `rust-toolchain.toml`.
- [Stellar CLI](https://github.com/stellar/stellar-cli/releases) 28.1.0
- `jq`, `python3`, `make`

## Testnet

```bash
./scripts/deploy.sh testnet
```

The script:

1. Creates the `folder-deployer` identity if it is missing and funds it via Friendbot. Override
   with `STELLAR_IDENTITY`, and set `ADMIN_IDENTITY` for a different admin.
2. Builds the optimized wasm (`make build-optimized`) and checks the template CIDs against
   `templates/manifest.json`.
3. Uploads the wasm and fails if the on-chain hash differs from the local sha256.
4. Deploys and calls `initialize`, then fails unless `get_admins()` returns exactly the admin.
5. Registers every template in `templates/manifest.json` with its metadata SHA-256 and checks
   `list_templates()`.
6. Writes `deployments/testnet.json` with the network, RPC URL, contract ID, wasm hash, admin,
   date, source commit, toolchain, tx hashes and templates.

Any failure stops the script with a non-zero exit code and the failing command.

Then run the end-to-end check:

```bash
./scripts/smoke-testnet.sh
```

To verify that the recorded source commit reproduces the deployed contract WASM:

```bash
make verify-deployment NETWORK=testnet
```

The verifier checks the recorded Rust and Stellar CLI versions, fetches the deployed WASM, builds
`source_commit` in a temporary Git worktree, and compares both SHA-256 hashes to `wasm_hash`.
It removes the temporary worktree on success or failure. The repository history must include
`source_commit`.

## Mainnet

`./scripts/deploy.sh mainnet` uses the same flow without Friendbot, so the identity must already
exist and be funded. Complete [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) first.

## Upgrades

There is no in-place upgrade. Deploy a new contract, then update `CONTRACT_ID` in
`Indexer-Frontend` and `Indexer-Backend` and re-index from the new contract.
