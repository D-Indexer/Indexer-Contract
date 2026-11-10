# Security Notes

## Authorization

| Function | Signer |
|---|---|
| `initialize` | `admin` (the first admin) |
| `add_admin`, `remove_admin`, `register_template` | `admin` argument, which must be in the admin list |
| `claim_folder`, `set_user_template`, `mint_portfolio`, `update_portfolio` | `owner` |
| All `get_*`, `list_templates`, `verify_portfolio` | none |

Tests assert the exact signer for every write and check that writes fail when nobody has signed
(`test_each_write_requires_the_right_signer`, `test_writes_fail_without_signature`).

## State invariants

- The contract can be initialized only once, and at least one admin must always remain.
- The admin list holds at most 10 entries; the template registry holds at most 256.
- One address holds at most one Folder name, and each name belongs to at most one address.
  Names are permanent: there is no release or transfer.
- An address mints at most one portfolio. A portfolio always shows the owner's currently selected
  template.
- Template IDs are non-zero and registered once.

## Known risks

- **Initialization front-running.** `initialize` is a separate transaction from deploy.
  `scripts/deploy.sh` checks `get_admins() == [admin]` afterwards and fails loudly if not; in that
  case, discard the contract. A constructor-based fix is tracked as an issue.
- **Single-signature admins.** Any one admin can add or remove other admins. Use multisig accounts
  as admins until two-step admin transfer or a threshold is implemented.
- **No upgrade path.** The contract has no `upgrade` entry point. A new version is a new
  contract ID.

## Operations

- Deploy keys are Stellar CLI identities stored in `~/.config/stellar`. Never commit them or
  pass secret keys on the command line.
- Add a new admin before removing an old one.
