# Validation Policy

The contract rejects unbounded user input before writing to ledger storage.

## Current bounds

| Field | Minimum length | Maximum length | Error |
|---|---:|---:|---|
| Folder name | 3 | 64 | `InvalidFolderName` |
| IPFS CID | 10 | 128 | `InvalidCID` |
| Template metadata URI | 10 | 256 | `InvalidMetadataURI` |

## Notes

- Folder names must use lowercase ASCII letters, digits, dots, or hyphens and must end with `.folder`.
- IPFS CIDs must start with `bafy` or `Qm`.
- Template metadata URIs must start with `ipfs://`.
- Frontend and backend validation should mirror these limits before submitting transactions.
- Future hardening should validate full CIDv0/CIDv1 structure rather than prefix only.
