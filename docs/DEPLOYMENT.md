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
5. Registers every template in `templates/manifest.json` and checks `list_templates()`.
6. Writes `deployments/testnet.json` with the network, RPC URL, contract ID, wasm hash, admin,
   date, source commit, toolchain, tx hashes and templates.

Any failure stops the script with a non-zero exit code and the failing command.

Then run the end-to-end check:

```bash
./scripts/smoke-testnet.sh
```

## Mainnet

`./scripts/deploy.sh mainnet` uses the same flow without Friendbot, so the identity must already
exist and be funded. Complete [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) first.

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
