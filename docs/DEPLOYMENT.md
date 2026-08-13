# Deployment Guide

## Prerequisites

- Rust stable + `wasm32v1-none` target
- [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli)
- A funded Stellar keypair for deployment

Validate local contract readiness before deploying:

```bash
make check
```

## Steps

Use [Release Checklist](RELEASE_CHECKLIST.md) for the full pre-deploy handoff.

1. Copy `.env.example` to `.env` and fill in `DEPLOYER_SECRET` and `ADMIN_ADDRESS`.
2. Run `./scripts/deploy.sh testnet` (or `mainnet`).
3. Save the printed `CONTRACT_ID` — set it in `Indexer-Frontend` and `Indexer-Backend` env files.

`STELLAR_RPC_URL` is optional for the deploy script. When present, the script passes it to Stellar CLI as `--rpc-url`; otherwise the CLI uses its configured network defaults.

## Manual Steps

See the Quick Start section in [README.md](../README.md).

## Upgrading the Contract

Soroban contracts are immutable once deployed. To upgrade:
1. Deploy a new contract instance.
2. Update `CONTRACT_ID` in all three repos.
3. Migrate any off-chain DB records in `Indexer-Backend` to point to the new contract.
