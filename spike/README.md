# Spike: split contracts (#249)

Prototype of the monolith split into three contracts in one Cargo workspace:

| Crate | Owns | Calls |
|---|---|---|
| `folder-registry` | Folder names (`claim_folder`, `get_folder`, `get_identity`) | — |
| `folder-templates` | Templates (`register_template`, `get_template`, `list_templates`) | — |
| `folder-portfolio` | Portfolio records (`mint_portfolio`, `update_portfolio`, `set_user_template`, `get_portfolio`, `verify_portfolio`) | `registry.get_identity`, `templates.get_template` |

Error codes keep the monolith's `ContractError` values. Run the ported tests
with `cargo test --workspace`.

## Measured overhead

`cargo build --workspace --target wasm32v1-none --release`, then
`cargo test -p folder-portfolio mint_cost -- --nocapture` (the test uses the
release WASM when present, so VM instantiation is included):

| `mint_portfolio` | CPU instructions | Memory bytes | Fee (stroops) |
|---|---|---|---|
| Monolith (1 contract) | 1,465,237 | 1,506,795 | 6,714,096 |
| Split (3 contracts, 2 cross-calls) | 1,314,637 | 3,829,680 | 5,113,520 |

Each cross-call instantiates another (small) WASM module, which more than
doubles memory. CPU and fee are lower because each small module is cheaper to
load than the monolith's single larger module and its larger instance entry,
which outweighs the two extra invocations. These are local test-environment
numbers; re-measure with `stellar contract invoke --sim-only` on testnet
before deciding.

## Decision record

**Costs.** ~2.5× memory per portfolio write; CPU and fee roughly flat to
lower. Three deployments, three instance TTLs to keep alive, and three
addresses for clients and the indexer.

**Risks.**
- Consistency across contracts: a Folder released or transferred in the
  registry is not visible to the portfolio contract until the next call that
  checks it; portfolios are keyed by address, so they outlive names.
- Cross-contract auth: the owner's signature now covers a tree of calls;
  wallets and simulation must handle it.
- Upgrade coupling moves to interfaces: changing `get_identity` or
  `get_template` signatures breaks `folder-portfolio`.
- Admin state is duplicated (templates and portfolio each have their own).

**Benefits.** Each concern upgrades independently, the registry is reusable by
other projects, and each WASM is smaller.

**Recommendation.** Keep the monolith for now. The split pays off once a
second consumer of the registry exists or upgrade frequency diverges between
concerns. When it does, split the registry first (highest reuse value) and
leave templates and portfolios together.

## Migration outline

1. Deploy `folder-registry` and `folder-templates`, then `folder-portfolio`
   initialized with their addresses.
2. Freeze writes on the monolith (upgrade to a version whose state-changing
   entry points return an error).
3. Copy state with admin-only import entry points (names, templates,
   portfolios), batched to stay within per-transaction limits, driven by the
   indexer's `folder_cl` / `tmpl_reg` / `port_mint` history and verified
   against monolith reads.
4. Point clients and the indexer at the new addresses; keep the frozen
   monolith readable as a fallback until all data is verified.
