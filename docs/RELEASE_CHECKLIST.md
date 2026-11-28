# Release Checklist

- [ ] `make check` passes and CI is green on the release commit.
- [ ] `Cargo.toml` version matches the `v*` tag; pushing the tag runs the `Release` workflow,
      which publishes the optimized wasm, `SHA256SUMS`, interface JSON, SBOM and a signed build
      provenance attestation (`gh attestation verify indexer_contract.wasm --repo D-Indexer/Indexer-Contract`).
- [ ] `make wasm-hash` matches the hash in the CI job summary for the same commit.
- [ ] `python3 scripts/template_cids.py --check` passes, and the template files are pinned on IPFS.
- [ ] `errors.json` is current (`make errors-json`); the release workflow attaches it.
- [ ] If `soroban-sdk`, the Stellar CLI or the Rust toolchain changed: run
      `./scripts/sdk-compat.sh <previous release tag>` and paste the report into the PR.
- [ ] `docs/EVENTS.md` matches `src/events.rs`, and the backend has been told about any event change.
- [ ] Deploy to testnet with `./scripts/deploy.sh testnet` and commit `deployments/testnet.json`.
- [ ] `./scripts/smoke-testnet.sh` passes against the new deployment.
- [ ] Update `CONTRACT_ID` in the `Indexer-Frontend` and `Indexer-Backend` environments.
- [ ] Mainnet only: the admin is a multisig account, the deploy identity is dedicated, and the
      mainnet readiness issue ([#127](https://github.com/D-Indexer/Indexer-Contract/issues/127))
      is closed.
- [ ] Mainnet only: `./scripts/mainnet-preflight.sh deployments/mainnet.json` reports
      **Overall: PASS** (multisig admins, wasm matches the tagged release, template CIDs
      retrievable, TTLs above the minimum, no admin is the deployer, CI green). It is read-only.
