# Indexer-Contract

[![Soroban Contract CI](https://github.com/Haroldwonder/Indexer-Contract/actions/workflows/contract-ci.yml/badge.svg)](https://github.com/Haroldwonder/Indexer-Contract/actions/workflows/contract-ci.yml)

Soroban smart contracts (Rust) powering the **Folder** ecosystem. Manages the on-chain template registry, user identity mapping, and portfolio NFT minting on the Stellar network.

## Overview

Folder is a dynamic on-chain identity platform that lets users create, own, and share verifiable portfolio NFTs on Stellar. Each "Folder" is a Non-Fungible Token whose metadata points to decentralized storage (IPFS), making portfolios portable, tamper-proof, and globally accessible at a fraction of a cent per transaction.

## Organisation

This contract is one of three repositories in the Folder organisation.

| Repo | Purpose | Language |
|---|---|---|
| `Indexer-Contract` | Soroban smart contracts — identity, templates, NFT minting | Rust |
| `Indexer-Frontend` | User-facing portfolio UI and passkey auth | TypeScript / React |
| `Indexer-Backend` | Off-chain indexer, API, IPFS pinning, event listener | TypeScript / Node.js |

## Domain Vocabulary

These terms are consistent across all three repos.

| Term | Definition |
|---|---|
| **Folder** | A user's on-chain portfolio identity — one per Stellar address |
| **Folder Name** | A unique human-readable handle (e.g. `alice.folder`) claimed on-chain |
| **Template** | A numbered layout (1–N) stored in the contract; maps to a visual design in the frontend |
| **Portfolio NFT** | The on-chain record linking an owner, template, and IPFS CID |
| **IPFS CID** | Content Identifier — the hash of portfolio data stored on IPFS, anchored on-chain |
| **Admin** | A Stellar address with elevated contract permissions |

## Features

- **Template Registry**: On-chain mapping of Template IDs to user profile data (bio, links, work history)
- **Portfolio NFTs**: Each portfolio is minted as an NFT with IPFS-backed metadata
- **Access Control**: Only the owner can update their Folder; anyone can verify its authenticity
- **Passkey Onboarding**: SEP-20 passkey support — sign up with thumbprint or FaceID, no seed phrases
- **IPFS Integration**: Large assets (resumes, screenshots) stored off-chain; CID anchored on-chain
- **Proof of Work**: Integrates Stellar payment history as verifiable work credentials
- **Event Emission**: On-chain events for off-chain indexing and frontend reactivity

## Architecture

```mermaid
graph TB
    subgraph Users
        U[User / Portfolio Owner]
        R[Recruiter / Client]
        D[Template Designer]
    end

    subgraph Frontend["Frontend (React)"]
        UI[Portfolio UI]
        PK[Passkey / Freighter Auth]
    end

    subgraph Contract["Smart Contracts (Soroban / Rust)"]
        REG[template_registry.rs]
        NFT[nft_mint.rs]
        ID[identity_map.rs]
        AUTH[access_control.rs]
        STOR[storage.rs]
        EVT[events.rs]
    end

    subgraph Backend["Backend (Node.js)"]
        IDX[indexer.ts]
        API[REST API]
        IPFS_SVC[IPFS Pinning]
        DB[(PostgreSQL)]
    end

    subgraph Stellar["Stellar Network"]
        LEDGER[Ledger]
        ASSET[Stellar Asset / NFT]
    end

    subgraph Decentralized["Decentralized Storage"]
        IPFS[IPFS / Filecoin]
    end

    U -->|claim Folder name| PK
    PK --> UI
    UI -->|mint / update portfolio| REG
    UI -->|mint NFT| NFT
    REG --> STOR
    NFT --> ASSET
    NFT -->|store CID| STOR
    ID --> STOR
    AUTH --> STOR
    STOR --> LEDGER
    LEDGER -->|events| EVT
    EVT -->|stream| IDX
    IDX --> DB
    IDX --> IPFS_SVC
    UI -->|POST /api/ipfs/pin| API
    API --> IPFS_SVC
    IPFS_SVC --> IPFS
    IPFS -->|CID| NFT
    R -->|verify portfolio| LEDGER
    D -->|publish template| REG
```

### Core Contract Modules

| File | Responsibility |
|---|---|
| `src/lib.rs` | Public contract entry point — all exposed functions |
| `src/types.rs` | `PortfolioNFT`, `Template`, `DataKey` enum |
| `src/errors.rs` | `ContractError` enum |
| `src/storage.rs` | All ledger reads/writes with TTL management |
| `src/access_control.rs` | `require_admin`, `require_owner`, admin CRUD |
| `src/identity_map.rs` | Folder name claiming and resolution |
| `src/template_registry.rs` | Template registration, listing, user assignment |
| `src/nft_mint.rs` | Portfolio NFT minting, updating, verification |
| `src/events.rs` | Event emission for all state changes |
| `src/test.rs` | 12-case test suite |

### Storage Model

- **Instance Storage**: Template definitions, platform config, admin list — shared contract-wide state
- **Persistent Storage**: User identity mappings, minted NFT records, IPFS CID anchors — per-user data with ~1 year TTL bump on every access

## Contract Functions

### Identity & Onboarding

- `claim_folder(owner, folder_name)` — Claim a unique Folder name (owner auth required; must be done before minting)
- `get_folder(folder_name)` → `Address` — Resolve a Folder name to its owner
- `get_identity(owner)` → `String` — Get the Folder name for an address

### Template Registry

- `register_template(admin, template_id, metadata_uri)` — Add a new template (admin only)
- `get_template(template_id)` → `Template` — Fetch template metadata
- `list_templates()` → `Vec<u32>` — All available Template IDs
- `set_user_template(owner, template_id)` — Assign a template to a Folder (owner auth required)
- `get_user_template(owner)` → `Option<u32>` — Get the template currently assigned to a Folder

### Portfolio NFT

- `mint_portfolio(owner, template_id, ipfs_cid)` — Mint a portfolio NFT (owner must have a Folder name; one mint per address)
- `update_portfolio(owner, ipfs_cid)` — Update the IPFS CID (owner auth required)
- `get_portfolio(owner)` → `PortfolioNFT` — Retrieve portfolio details
- `verify_portfolio(owner)` → `bool` — Public read — `true` when a portfolio exists, `false` otherwise

### Admin

- `initialize(admin)` — One-time contract setup
- `add_admin(admin, new_admin)` — Grant admin role
- `remove_admin(admin, target)` — Revoke admin role (cannot remove last admin)

## Frontend Integration

The frontend calls the contract via `@stellar/stellar-sdk` / Soroban RPC. All write functions require the caller to sign with their Stellar keypair or SEP-20 passkey — never handle raw secret keys in the frontend.

### Events the Frontend Listens For

| Symbol | Payload | Action |
|---|---|---|
| `folder_cl` | `(owner, folder_name)` | Update claimed-name UI state |
| `port_mint` | `(owner, template_id, ipfs_cid)` | Show new portfolio card |
| `port_upd` | `(owner, ipfs_cid)` | Refresh portfolio display |
| `tmpl_reg` | `(template_id, metadata_uri)` | Add template to picker |
| `tmpl_set` | `(owner, template_id)` | Highlight active template |

> Event symbols are max 9 characters — Soroban `symbol_short!` limit.

## Backend Integration

The backend indexes contract events via Soroban RPC event streaming (entry point: `src/indexer.ts`).

### Events → Backend Actions

| Symbol | Backend Action |
|---|---|
| `folder_cl` | Upsert identity record in DB |
| `port_mint` | Pin IPFS CID, store NFT record |
| `port_upd` | Re-pin new CID, update NFT record |
| `tmpl_reg` | Cache template metadata |
| `tmpl_set` | Update user-template mapping |

### Backend API Endpoints

```
GET  /api/folder/:folder_name   → owner address + portfolio
GET  /api/portfolio/:owner      → full portfolio (contract + IPFS merged)
GET  /api/templates             → all templates with metadata
GET  /api/identity/:owner       → folder name for an address
POST /api/ipfs/pin              → pin portfolio JSON, returns CID
```

### IPFS Flow

```
1. Frontend  →  POST /api/ipfs/pin  (portfolio JSON)
2. Backend   →  pins to IPFS, returns CID
3. Frontend  →  mint_portfolio(owner, template_id, CID)  on contract
4. Contract  →  emits port_mint event
5. Backend   →  indexes event, stores CID reference in DB
```

> IPFS pinning always happens **before** the contract transaction, not after.

## Shared Data Structures

```ts
// TypeScript — Frontend & Backend
interface PortfolioNFT {
  owner: string;        // Stellar address (G...)
  template_id: number;
  ipfs_cid: string;     // e.g. "bafybeig..."
  minted_at: number;    // Unix timestamp (ledger)
}

interface Template {
  template_id: number;
  metadata_uri: string; // IPFS URI pointing to template JSON
  creator: string;      // Stellar address
}
```

```rust
// Rust — src/types.rs
pub struct PortfolioNFT { pub owner, pub template_id, pub ipfs_cid, pub minted_at }
pub struct Template     { pub template_id, pub metadata_uri, pub creator }
```

## Security Features

1. **Owner-Only Writes**: All portfolio mutations require the owner's Stellar signature
2. **Folder-First Minting**: A Folder name must be claimed before a portfolio can be minted
3. **No Double-Mint**: A second `mint_portfolio` call returns `PortfolioAlreadyMinted`; use `update_portfolio`
4. **Public Verification**: Anyone can read and verify portfolio authenticity without permission
5. **Immutable CID Anchoring**: IPFS CIDs stored on-chain cannot be silently swapped
6. **Admin Role Guards**: Template registration gated behind admin authorization
7. **TTL Management**: Persistent storage entries are bumped ~1 year on every access to prevent expiry

## Why Stellar

| Property | Detail |
|---|---|
| Transaction cost | ~$0.00001 — accessible to job seekers in any economy |
| Finality | ~5 seconds — portfolio updates are live globally almost immediately |
| Interoperability | Stellar payment history (USDC, fiat anchors) can be surfaced as Proof of Work |
| Passkeys (SEP-20) | Biometric onboarding — no 24-word seed phrases for non-crypto users |

## Roadmap

| Phase | Goal | Key Tech |
|---|---|---|
| Phase 1 | The Identity | Users create a Stellar address (Freighter / Passkeys) and claim their Folder name |
| Phase 2 | Template Engine | 3–5 Rust-based templates for Dev, Design, Admin industries |
| Phase 3 | Verification | Link GitHub, LinkedIn, or on-chain credentials to a Folder |
| Phase 4 | The Marketplace | Designers create and sell new portfolio templates as digital assets |

## Quick Start

### 1. Build the Contract

```bash
cargo build --target wasm32v1-none --release
stellar contract optimize --wasm target/wasm32v1-none/release/indexer_contract.wasm
```

### 2. Deploy to Testnet

```bash
stellar contract deploy \
  --wasm target/wasm32v1-none/release/indexer_contract.optimized.wasm \
  --source deployer \
  --network testnet
```

### 3. Initialize

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source deployer \
  --network testnet \
  -- initialize \
  --admin <ADMIN_ADDRESS>
```

### 4. Claim a Folder

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source <USER_KEYPAIR> \
  --network testnet \
  -- claim_folder \
  --owner <USER_ADDRESS> \
  --folder_name "jane.folder"
```

### 5. Mint a Portfolio NFT

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source <USER_KEYPAIR> \
  --network testnet \
  -- mint_portfolio \
  --owner <USER_ADDRESS> \
  --template_id 1 \
  --ipfs_cid "bafybeig..."
```

## Environment Variables

### This repo (deploy scripts)

```env
STELLAR_NETWORK=testnet
STELLAR_RPC_URL=https://soroban-testnet.stellar.org
DEPLOYER_SECRET=S...
CONTRACT_ID=C...
```

### Indexer-Backend

```env
CONTRACT_ID=C...
STELLAR_RPC_URL=https://soroban-testnet.stellar.org
STELLAR_NETWORK=testnet
IPFS_API_URL=https://api.pinata.cloud
IPFS_API_KEY=...
DATABASE_URL=postgresql://...
```

### Indexer-Frontend

```env
NEXT_PUBLIC_CONTRACT_ID=C...
NEXT_PUBLIC_STELLAR_NETWORK=testnet
NEXT_PUBLIC_BACKEND_URL=https://api.folder.xyz
```

## Testing

```bash
cargo test
```

## Local Quality Gates

Run the same checks locally before opening a PR:

```bash
make check
```

This runs formatting, `cargo check`, clippy with warnings denied, the unit test suite, and the release WASM build.

Tests cover:
- ✅ Contract initialization and double-init rejection
- ✅ Folder name claiming and collision prevention
- ✅ Mint rejected without a claimed Folder name
- ✅ Template registration and retrieval
- ✅ Portfolio NFT minting and CID anchoring
- ✅ Double-mint rejection
- ✅ Portfolio update authorization
- ✅ Public verification reads
- ✅ Admin role management
- ✅ Unauthorized access rejection
- ✅ User template assignment and retrieval

## Error Codes

Input validation limits are documented in [Validation Policy](docs/VALIDATION.md).

| Code | Error | Description | Frontend / Backend Handling |
|---|---|---|---|
| 1 | AlreadyInitialized | Contract already initialized | Ignore on deploy retry |
| 2 | NotInitialized | Contract not yet initialized | Block all UI until admin calls `initialize` |
| 3 | FolderNameTaken | Folder name already claimed | Show "name unavailable" in claim form |
| 4 | FolderNotFound | Folder name does not exist | Show 404 portfolio page |
| 5 | PortfolioNotFound | No portfolio minted for this address | Show "no portfolio yet" CTA |
| 6 | TemplateNotFound | Template ID does not exist | Remove stale template from picker |
| 7 | Unauthorized | Caller is not the owner or admin | Show auth error, prompt re-sign |
| 8 | InvalidCID | IPFS CID is empty or malformed | Reject upload before submitting tx |
| 9 | InvalidFolderName | Folder name is empty or invalid | Validate client-side before tx |
| 10 | AdminAlreadyExists | Admin address already registered | Suppress duplicate admin UI action |
| 11 | AdminNotFound | Admin address not found | Show "admin not found" in admin panel |
| 12 | CannotRemoveLastAdmin | Cannot remove the last remaining admin | Disable remove button when only 1 admin |
| 13 | PortfolioAlreadyMinted | Portfolio already minted | Direct user to `update_portfolio` |
| 14 | InvalidMetadataURI | Template metadata URI is empty, malformed, or too long | Reject template registration before tx |
| 15 | TemplateAlreadyExists | Template ID already registered | Ask admin to choose a new template ID |
| 16 | FolderAlreadyClaimed | Address already owns a Folder name | Show existing Folder and disable new claim |

## Events

| Symbol | Emitted When | Payload |
|---|---|---|
| `folder_cl` | A user claims a Folder name | `(owner, folder_name)` |
| `port_mint` | A new portfolio NFT is minted | `(owner, template_id, ipfs_cid)` |
| `port_upd` | A portfolio's IPFS CID is updated | `(owner, ipfs_cid)` |
| `tmpl_reg` | A new template is added to the registry | `(template_id, metadata_uri)` |
| `tmpl_set` | A user selects a template for their Folder | `(owner, template_id)` |

## Cross-Repo Change Checklist

When a contract function signature changes:
- [ ] Update `src/lib.rs` in `Indexer-Contract`
- [ ] Update `src/lib/contract.ts` in `Indexer-Frontend`
- [ ] Update API handler + indexer in `Indexer-Backend`
- [ ] Update the function table and data structures in this README

When a new event is added:
- [ ] Add emission in `src/events.rs`
- [ ] Add to frontend event listener
- [ ] Add to backend indexer
- [ ] Update the events table in this README

## Dependencies

- `soroban-sdk = "25.3.1"`

## License

MIT

## Contributing

Contributions are welcome!

See [Development Workflow](docs/DEVELOPMENT.md) for local tooling and quality gates.

Quick checklist:
- All tests pass: `cargo test`
- New features include tests
- Documentation is updated
- Contract function changes are reflected in all three repos (see checklist above)

## Support

- GitHub Issues: [Create an issue](https://github.com/Haroldwonder/Indexer-Contract/issues)
- Stellar Discord: https://discord.gg/stellar
