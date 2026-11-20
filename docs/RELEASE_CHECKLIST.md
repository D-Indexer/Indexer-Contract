# Release Checklist

- [ ] `make check` passes and CI is green on the release commit.
- [ ] `make wasm-hash` matches the hash in the CI job summary for the same commit.
- [ ] `python3 scripts/template_cids.py --check` passes, and the template files are pinned on IPFS.
- [ ] `docs/EVENTS.md` matches `src/events.rs`, and the backend has been told about any event change.
- [ ] Deploy to testnet with `./scripts/deploy.sh testnet` and commit `deployments/testnet.json`.
- [ ] `./scripts/smoke-testnet.sh` passes against the new deployment.
- [ ] Update `CONTRACT_ID` in the `Indexer-Frontend` and `Indexer-Backend` environments.
- [ ] Add the released wasm as an upgrade fixture: build it (`make build-wasm`), copy
      `target/wasm32v1-none/release/indexer_contract.wasm` to
      `tests/fixtures/wasm/indexer_contract_v<version>.wasm`, point `PREVIOUS_RELEASE_WASM` in
      `src/test.rs` at it, and extend the upgrade test to populate any new `DataKey` variants.
- [ ] Mainnet only: the admin is a multisig account, the deploy identity is dedicated, and the
      mainnet readiness issue ([#127](https://github.com/D-Indexer/Indexer-Contract/issues/127))
      is closed.
