# Storage Model

## Instance storage

Instance storage holds contract-wide values:

- Initialization flag
- Admin list
- Template definitions
- Template ID list

## Persistent storage

Persistent storage holds user-specific values:

- Folder name to owner mapping
- Owner to Folder name mapping
- Owner to selected template mapping
- Owner to portfolio record mapping

Persistent entries are extended on access using the TTL constants in `src/constants.rs`.
