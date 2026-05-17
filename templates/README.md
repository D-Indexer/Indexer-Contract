# Templates

This directory holds the metadata JSON for each portfolio template registered on-chain.

Each file corresponds to a `template_id` and is pinned to IPFS. The resulting CID is passed to `register_template` as `metadata_uri`.

## Format

```json
{
  "template_id": 1,
  "name": "Developer",
  "description": "Clean single-page layout for software engineers.",
  "preview_image": "ipfs://...",
  "fields": ["bio", "github", "skills", "projects", "work_history"]
}
```

## Current Templates

| ID | Name | Target Industry | Status |
|---|---|---|---|
| 1 | Developer | Software Engineering | Planned |
| 2 | Designer | UI/UX & Creative | Planned |
| 3 | Admin | Operations & Management | Planned |
