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
   date, source commit, toolchain, tx hashes, ledgers and templates, after validating it against
   [`deployments/schema.json`](../deployments/schema.json).

Any failure stops the script with a non-zero exit code and the failing command.

### Resuming a failed deploy

Progress is saved to `deploy-out/<network>/state.json` after every step (wasm hash, contract ID,
and the tx hash and ledger of each completed step). If a run fails, rerun with `--resume`: completed
steps are skipped, the same contract ID is reused, and only templates missing from
`list_templates()` are registered. A template that is already on-chain with a different
`metadata_uri` than the manifest stops the script. Use `--fresh` to discard the state and deploy a
new contract. Running without either flag while a state file exists is refused.

### Deployment record

`deployments/<network>.json` follows [`deployments/schema.json`](../deployments/schema.json)
(JSON Schema 2020-12). CI validates every record with `python3 scripts/validate_deployments.py`.

Indexers should start `getEvents` at `deploy_ledger`, the ledger of the deploy transaction.
`ledgers` holds the ledger of each entry in `transactions`, and every template has a `ledger`.
The values come from RPC `getTransaction`, or from Horizon once the transaction is older than RPC
retention. To add them to an older record:

```bash
python3 scripts/backfill_ledgers.py backfill deployments/testnet.json
```

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

### Offline or multisig signing

The mainnet admin should be a multisig or hardware-backed account whose keys never touch the
deploy machine. With `ADMIN_ADDRESS` set to that account (it is also the source of every
transaction), the deploy runs one step at a time: `upload_wasm`, `deploy`, `initialize`, then
`template-<id>` for each manifest entry.

```bash
export ADMIN_ADDRESS=G...
./scripts/deploy.sh mainnet --build-only        # writes deploy-out/mainnet/<step>.xdr and <step>.txt
stellar tx sign --sign-with-key <signer> --network mainnet < deploy-out/mainnet/<step>.xdr > signed.xdr
./scripts/deploy.sh mainnet --submit signed.xdr # submits, verifies and records the step
# repeat --build-only / sign / --submit until the script writes deployments/mainnet.json
```

Each step is built only after the previous one is on-chain, so its sequence number is current
when it is signed. `<step>.txt` lists the operation, arguments and fee for signers. The deploy
step uses a random salt, so the contract ID is known when the XDR is built. After each submit the
script runs the same checks as a local deploy: the on-chain wasm hash, `get_admins` and the
template URIs, then `list_templates` after the last template.

## Upgrades

There is no in-place upgrade. Deploy a new contract, then update `CONTRACT_ID` in
`Indexer-Frontend` and `Indexer-Backend` and re-index from the new contract.

## Reproducible build (Docker)

`make docker-build` builds the optimized wasm inside the pinned `linux/amd64`
image from the `Dockerfile` (base image by digest, rustup and Stellar CLI
checksum-verified, non-root user, fixed paths) and prints its sha256. Apple
Silicon runs the same image under emulation. `.devcontainer/` opens the `dev`
stage of the same image. CI checks that the Docker and native hashes match.

Expected hash: at `8fc9510` both `make docker-build` and the native
`make build-optimized` produced
`b262c3163359fb81adb20f0c6d6623579a5df02ee6221f4bf31d76487fb32055` on
x86_64 Linux. The current `main` does not compile (see the PR description),
so its hash will be recorded once that is fixed.
