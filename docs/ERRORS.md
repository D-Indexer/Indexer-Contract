# Error Reference

Contract errors are defined in `src/errors.rs` and are stable numeric values for client handling.

Client applications should map each error to a user-actionable message and avoid parsing panic strings or RPC transport errors as contract logic failures.

When adding a new error:

1. Append it to `ContractError` without renumbering existing variants.
2. Add at least one exact-error unit test.
3. Update the error table in `README.md`.
