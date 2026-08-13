# Contract Events

Events are emitted through typed Soroban `#[contractevent]` structs in `src/events.rs`.

| Event | Topic | Data |
|---|---|---|
| `FolderClaimed` | `folder_cl` | `owner`, `folder_name` |
| `PortfolioMinted` | `port_mint` | `owner`, `template_id`, `ipfs_cid` |
| `PortfolioUpdated` | `port_upd` | `owner`, `ipfs_cid` |
| `TemplateRegistered` | `tmpl_reg` | `template_id`, `metadata_uri` |
| `TemplateAssigned` | `tmpl_set` | `owner`, `template_id` |

The short topic symbols are intentionally preserved for frontend and backend indexer compatibility.
