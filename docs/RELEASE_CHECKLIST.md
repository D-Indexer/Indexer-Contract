# Release Checklist

Use this before deploying a new contract instance.

- [ ] Run `make check`.
- [ ] Run `make optimize-wasm` in an environment with Stellar CLI installed.
- [ ] Confirm `DEPLOYER_SECRET` belongs to the intended deployer account.
- [ ] Confirm `ADMIN_ADDRESS` is the intended initial admin.
- [ ] Deploy with `./scripts/deploy.sh testnet` first.
- [ ] Record the emitted `CONTRACT_ID`.
- [ ] Update `Indexer-Frontend` and `Indexer-Backend` environment files.
- [ ] Run a smoke test against the deployed contract.
- [ ] Repeat for `mainnet` only after testnet validation passes.
