# Validation Policy

The contract rejects unbounded user input before writing to ledger storage.

## Current bounds

| Field | Minimum length | Maximum length | Error |
|---|---:|---:|---|
| Folder name | 3 | 64 | `InvalidFolderName` |
| IPFS CID | 10 | 128 | `InvalidCID` |
| Template metadata URI | 10 | 256 | `InvalidMetadataURI` |

## Notes

- These checks are intentionally conservative length bounds.
- Frontend and backend validation should mirror these limits before submitting transactions.
- Future hardening should validate allowed folder-name characters and CID structure.
