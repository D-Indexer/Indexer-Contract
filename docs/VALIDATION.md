# Validation Policy

All user-supplied strings are bounded and format-checked in
[`src/validation.rs`](../src/validation.rs) before anything is written to ledger storage. Clients
should mirror these rules before submitting transactions.

| Input | Rule | Error |
|---|---|---|
| Folder name | `<label>.folder`, at most 64 bytes in total. The label is 3+ bytes of `[a-z0-9-]`, does not start or end with `-`, does not have `-` as both its 3rd and 4th bytes (rejects `xn--` and other IDNA A-label prefixes), and is not all-numeric. No dots inside the label. | `InvalidFolderName` (9) |
| IPFS CID | CIDv0: `Qm` + base58btc, exactly 46 chars. **or** CIDv1: `ba` + lowercase base32 (`[a-z2-7]`), 46–128 chars. | `InvalidCID` (8) |
| Template metadata URI | `ipfs://<CID>` optionally followed by `/<path>`, where the CID follows the rule above and the path uses `[A-Za-z0-9._~/-]`. At most 256 bytes. | `InvalidMetadataURI` (14) |
| Template ID | Non-zero `u32` | `InvalidTemplateID` (17) |

Examples:

- Accepted: `jane-doe.folder`, `a-b-c.folder`, `agent007.folder`,
  `bafkreibm6jg3ux5qumhcn2b3flc3tyu6dmlb4xa7u5bf44yegnrjhc4yeq`,
  `ipfs://bafkrei…/templates/1.json`.
- Rejected: `.folder`, `ab.folder`, `-abc.folder`, `a.b.folder`, `Alice.folder`, `xn--abc.folder`,
  `ab--c.folder`, `007.folder` (all-numeric labels are reserved), `bafy_not_a_cid`, uppercase CIDv1,
  `https://…`, `ipfs://<cid>?q=1`.

## Folder label policy

- Hyphens are allowed, but a label with `-` as both its 3rd and 4th bytes is rejected. This covers
  the IDNA A-label prefix `xn--` (punycode) and other ambiguous `ab--c` shapes that can be confused
  with encoded labels.
- All-numeric labels (e.g. `007.folder`) are rejected: they are reserved for future system use and
  are easy to confuse with numeric identifiers.
- Validation applies to new claims only. Names already claimed on-chain stay valid and resolvable;
  the contract does not re-validate or migrate existing entries.

## Non-goals

- The contract checks the textual shape of a CID, not its multihash or codec bytes.
- It does not fetch IPFS content, check pinning, or parse metadata JSON.
