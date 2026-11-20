# Validation Policy

All user-supplied strings are bounded and format-checked in
[`src/validation.rs`](../src/validation.rs) before anything is written to ledger storage. Clients
should mirror these rules before submitting transactions.

| Input | Rule | Error |
|---|---|---|
| Folder name | `<label>.folder`, at most 64 bytes in total. The label is 3+ bytes of `[a-z0-9-]` and does not start or end with `-`. No dots inside the label. | `InvalidFolderName` (9) |
| IPFS CID | CIDv0: `Qm` + base58btc, exactly 46 chars. **or** CIDv1: `ba` + lowercase base32 (`[a-z2-7]`), 46–128 chars. | `InvalidCID` (8) |
| Template metadata URI | `ipfs://<CID>` optionally followed by `/<path>`, where the CID follows the rule above and the path uses `[A-Za-z0-9._~/-]`. At most 256 bytes. | `InvalidMetadataURI` (14) |
| Template ID | Non-zero `u32` | `InvalidTemplateID` (17) |

Examples:

- Accepted: `jane-doe.folder`, `007.folder`, `bafkreibm6jg3ux5qumhcn2b3flc3tyu6dmlb4xa7u5bf44yegnrjhc4yeq`,
  `ipfs://bafkrei…/templates/1.json`.
- Rejected: `.folder`, `ab.folder`, `-abc.folder`, `a.b.folder`, `Alice.folder`, `bafy_not_a_cid`,
  uppercase CIDv1, `https://…`, `ipfs://<cid>?q=1`.

## Formal verification (Kani)

The validators are implemented as pure `&[u8]` functions in
[`src/validation_core.rs`](../src/validation_core.rs); `src/validation.rs` only copies the contract
`String` into a bounded stack buffer and calls them. Run the proofs with:

```bash
cargo install --locked kani-verifier && cargo kani setup   # once
cargo kani
```

| Harness | Property | Input bound |
|---|---|---|
| `folder_name_never_panics_and_matches_grammar` | Never panics. Every accepted name is `<label>.folder`, 10–64 bytes, the label is all `[a-z0-9-]`, and its first and last bytes are never `-`. | all inputs of 0–16 bytes |
| `cid_never_panics_and_has_valid_shape` | Never panics. Every accepted CID starts with `Qm` and is exactly 46 base58btc bytes, or starts with `ba` and is 46–128 lowercase base32 bytes. | all inputs of 0–48 bytes |
| `metadata_uri_never_panics` | Never panics, and every accepted URI starts with `ipfs://`. | all inputs of 0–24 bytes |

The bounds keep the proofs tractable. The 48-byte CID bound covers every CIDv0 and the shortest
CIDv1 strings. A scheduled CI workflow (`.github/workflows/kani.yml`) runs the proofs weekly.

## Non-goals

- The contract checks the textual shape of a CID, not its multihash or codec bytes.
- It does not fetch IPFS content, check pinning, or parse metadata JSON.
