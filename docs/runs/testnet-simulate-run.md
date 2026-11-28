# Testnet simulate run (`smoke-testnet.sh --simulate`)

Recorded 2026-11-20 with Stellar CLI 28.1.0 against `deployments/testnet.json`:

```sh
SIM_MINT_OWNER=GAKWF44LBBRVG4NWTJJM4YIT5ZVUCQ4HTAJUJOEODELGVBMEIDRJ23DB \
  SIM_PORTFOLIO_OWNER=GAPAMTTVHB7RZ5DQHEINZN3GQYRCXXWNCQNQSY5YAU6RKQUYX2QJT2CM \
  SMOKE_MARKDOWN=1 ./scripts/smoke-testnet.sh --simulate
```

`SIM_MINT_OWNER` owns a folder but no portfolio; `SIM_PORTFOLIO_OWNER` is the account from the recorded smoke run in [testnet-smoke-run.md](../testnet-smoke-run.md). Read-back assertions are skipped because simulated steps cannot see each other's writes. The source account's sequence number was the same before and after the run, so no transaction was submitted.

- Mode: simulate (`--send=no`, nothing submitted)
- Network: `testnet` (`https://soroban-testnet.stellar.org`)
- Contract: [`CDDQK74SMH4RB4HYLHM6BYZPTAMDMRISHW5Q274KADVZVFD57K43O6KV`](https://stellar.expert/explorer/testnet/contract/CDDQK74SMH4RB4HYLHM6BYZPTAMDMRISHW5Q274KADVZVFD57K43O6KV)
- Source: `GABPCJQTLNW7KOFYWISABLQ3HV4GKW5UO57EAA2MINRIVBBG43LFSXWW` (sequence `25036575549161472`)
- Folder: `smoke-1795170142.folder`, template: `1`

| # | Call | Owner | Result | Resource fee |
|---|---|---|---|---|
| 1 | `claim_folder` | `GBT2BFFNSXRYDWCFRRNMYXCPIF7X4FS5NAOEZHOMUXJYL4IQWG7C5QJ6` | ok (read-back skipped) | 0.0917568 XLM |
| 2 | `mint_portfolio` | `GAKWF44LBBRVG4NWTJJM4YIT5ZVUCQ4HTAJUJOEODELGVBMEIDRJ23DB` | ok (read-back skipped) | 0.1292946 XLM |
| 3 | `update_portfolio` | `GAPAMTTVHB7RZ5DQHEINZN3GQYRCXXWNCQNQSY5YAU6RKQUYX2QJT2CM` | ok (read-back skipped) | 0.0146782 XLM |
| 4 | `verify_portfolio` | `GAPAMTTVHB7RZ5DQHEINZN3GQYRCXXWNCQNQSY5YAU6RKQUYX2QJT2CM` | returned `true` | 0.0006734 XLM |

OK: simulate run passed; source sequence unchanged (25036575549161472), 0 transactions submitted
