# Templates

This directory holds the metadata JSON for each portfolio template registered on-chain.

Each file corresponds to a `template_id` and is pinned to IPFS. The resulting CID is passed to `register_template` as `metadata_uri`.

## Files

| File | Purpose |
|---|---|
| `<id>.json` | Template metadata for `template_id = <id>` |
| `manifest.json` | `template_id` → CID / `metadata_uri` registered by `scripts/deploy.sh` |

The CID of each file is a CIDv1 (raw codec, sha2-256), identical to what
`ipfs add --cid-version 1 templates/<id>.json` produces. Recompute or verify:

```bash
python3 scripts/template_cids.py templates/1.json
python3 scripts/template_cids.py --check   # verifies manifest.json
```

`deploy.sh` refuses to register templates if the manifest does not match the files.

> **Pinning:** the CIDs are content hashes of the committed files. They are
> only retrievable from IPFS once someone pins those exact files, e.g.
> `ipfs add --cid-version 1 templates/*.json` or a pinning service upload of
> the same bytes. Editing a file changes its CID, so update `manifest.json`
> and register a new template ID instead.

## Format

```json
{
  "schema_version": 1,
  "template_id": 1,
  "name": "Developer",
  "description": "Clean single-page layout for software engineers.",
  "target_industry": "Software Engineering",
  "fields": ["bio", "github", "skills", "projects", "work_history"]
}
```

## Current Templates

| ID | Name | Target Industry |
|---|---|---|
| 1 | Developer | Software Engineering |
| 2 | Designer | UI/UX & Creative |
| 3 | Operations | Operations & Management |
