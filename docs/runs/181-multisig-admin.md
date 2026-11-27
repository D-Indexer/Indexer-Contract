# Multisig Admin Run

Output of `MULTISIG_MARKDOWN=1 ./scripts/multisig-admin-demo.sh` on testnet, run on
2026-11-20 (UTC). The script creates three Friendbot-funded identities, turns the first into a
2-of-3 classic multisig account (`set_options`: master weight 1, two signers weight 1, low/medium/
high thresholds 2), deploys a throwaway contract, initializes it with the multisig account as
admin, then registers a template from a `--build-only` envelope.

Two signing attempts are made from the same envelope: one signature (signer A only) is rejected
with `tx_bad_auth`, two signatures (A and B) succeed. The script exits non-zero on any unexpected
result, so a passing run is the evidence that the one-signature attempt failed.

- Network: `testnet` (`https://soroban-testnet.stellar.org`)
- Multisig account: `G...` (2-of-3, master weight 1)
- Signers: `G...` (A), `G...` (B)
- Throwaway contract: `C...`

| # | Step | Result | Transaction |
|---|---|---|---|
| 1 | `set_options` | 2-of-3 signers installed, thresholds = 2 | `…` |
| 2 | `upload_wasm` | wasm hash `…` | `…` |
| 3 | `deploy_contract` | contract `C…` | `…` |
| 4 | `initialize` | admin = multisig account | `…` |
| 5 | `register_template (1 signature)` | rejected as expected (`tx_bad_auth`) | `…` |
| 6 | `register_template` | signed by 2 of 3 signers | `…` |

## Re-running

```bash
./scripts/multisig-admin-demo.sh                    # plain-text output
MULTISIG_MARKDOWN=1 ./scripts/multisig-admin-demo.sh # this format
```

The run needs a funded testnet account only through Friendbot, so no secrets are required. The
throwaway contract is not recorded in `deployments/`; it is safe to abandon.
