# Security Notes

## Authorization

- `initialize` requires the initial admin address to authorize.
- Admin mutations and template registration require an authorized admin.
- Folder, template-selection, mint, and update writes require owner authorization.
- Public reads do not require authorization.

## State invariants

- A contract can be initialized only once.
- At least one admin must remain.
- One address can claim at most one Folder name.
- One Folder name can be claimed by at most one address.
- A portfolio can be minted at most once per owner.
- A template ID can be registered at most once.
- Template IDs must be non-zero.
- User template selection requires a claimed Folder.

## Input handling

User-provided strings are bounded and format-checked before storage. See `docs/VALIDATION.md`.
