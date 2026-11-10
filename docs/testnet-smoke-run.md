# Testnet Smoke Run

Output of `SMOKE_MARKDOWN=1 ./scripts/smoke-testnet.sh` against the contract recorded in
[`deployments/testnet.json`](../deployments/testnet.json), run on 2026-11-09 (UTC).

The script creates a fresh Friendbot-funded account, then claims a Folder, mints a portfolio,
updates its CID and verifies it. After each write it reads the state back (`get_folder`,
`get_portfolio`) and exits non-zero on any mismatch. `verify_portfolio` is read-only, so it is
submitted with `--send=yes` to give it a transaction hash. All five transactions for the test
account (Friendbot account creation plus the four below) show as successful on Horizon.

The portfolio CIDs are CIDv1 hashes of small JSON documents the script generates. They are
real content hashes but were not pinned, so they will not resolve on a public gateway.

- Network: `testnet` (`https://soroban-testnet.stellar.org`)
- Contract: [`CDDQK74SMH4RB4HYLHM6BYZPTAMDMRISHW5Q274KADVZVFD57K43O6KV`](https://stellar.expert/explorer/testnet/contract/CDDQK74SMH4RB4HYLHM6BYZPTAMDMRISHW5Q274KADVZVFD57K43O6KV)
- Test account: [`GAPAMTTVHB7RZ5DQHEINZN3GQYRCXXWNCQNQSY5YAU6RKQUYX2QJT2CM`](https://stellar.expert/explorer/testnet/account/GAPAMTTVHB7RZ5DQHEINZN3GQYRCXXWNCQNQSY5YAU6RKQUYX2QJT2CM) (fresh, Friendbot-funded)
- Folder: `smoke-1794262900.folder`, template: `1`
- Portfolio CIDs: v1 `bafkreie33ewzcjvpo3gldpvqgevtoy4oaygkqri3cwxqr6o6lsqc4jqvwu`, v2 `bafkreifwfa7v77kn2om36djjtqiwqykldpady3ytsfbqkqgcrfilhjqtnu`

| # | Call | Result | Transaction |
|---|---|---|---|
| 1 | `claim_folder` | get_folder(`smoke-1794262900.folder`) = owner | [`3f659a4283badbb3ee509652b8fbb1ebf0c80124b02a0485f6695f830f94dc40`](https://stellar.expert/explorer/testnet/tx/3f659a4283badbb3ee509652b8fbb1ebf0c80124b02a0485f6695f830f94dc40) |
| 2 | `mint_portfolio` | ipfs_cid = v1, template_id = 1 | [`3805cb43452b15ba5962085b6d17d8591700a827a48caa4040854da4799b5ccb`](https://stellar.expert/explorer/testnet/tx/3805cb43452b15ba5962085b6d17d8591700a827a48caa4040854da4799b5ccb) |
| 3 | `update_portfolio` | ipfs_cid = v2 | [`e745547d7e2c1be63d370de6fdc3b6277b1311ab7e578b558161d31276f4e527`](https://stellar.expert/explorer/testnet/tx/e745547d7e2c1be63d370de6fdc3b6277b1311ab7e578b558161d31276f4e527) |
| 4 | `verify_portfolio` | returned `true` | [`2e5f3c4fc8728502bfee3b654961b870a5ab0d84bcefe804c5441329edfaaf91`](https://stellar.expert/explorer/testnet/tx/2e5f3c4fc8728502bfee3b654961b870a5ab0d84bcefe804c5441329edfaaf91) |

## Re-running

```bash
./scripts/smoke-testnet.sh                   # plain-text output
SMOKE_MARKDOWN=1 ./scripts/smoke-testnet.sh  # this format
```

It can also be triggered manually from GitHub Actions (`Soroban Contract CI` → *Run workflow*).

> Note: an earlier run on the same day (account `GD6UL6WJ…JQGP`) passed every on-chain check, but
> the script attributed the `update_portfolio` hash to `verify_portfolio` because of a subshell bug
> in hash capture. The bug is fixed; the run above is the first one with correct hashes.
