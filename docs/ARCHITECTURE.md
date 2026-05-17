# Architecture Notes

## Module Boundaries

Each `src/` module owns one concern. `lib.rs` is the only public surface — it delegates everything.

| Module | Owns |
|---|---|
| `identity_map` | Folder name ↔ address mapping |
| `template_registry` | Template CRUD + user assignment |
| `nft_mint` | Portfolio NFT lifecycle |
| `access_control` | Auth helpers (`require_admin`, `require_owner`) |
| `storage` | All ledger I/O + TTL management |
| `events` | Event emission only — no logic |

## Storage Strategy

- **Instance storage** for shared/global state (admin list, template registry, config). Cheap, no TTL concern.
- **Persistent storage** for per-user data (identity, portfolio, template selection). Every read/write bumps TTL ~1 year to prevent expiry.

## Adding a New Feature

1. Add any new storage keys to `DataKey` in `src/types.rs`.
2. Add read/write helpers in `src/storage.rs` with TTL bumps if persistent.
3. Implement logic in the relevant module (or a new module).
4. Expose via `src/lib.rs`.
5. Add event emission in `src/events.rs`.
6. Add tests in `src/test.rs`.
7. Update the Cross-Repo Change Checklist in `README.md`.
