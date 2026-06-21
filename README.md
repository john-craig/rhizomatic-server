# Rhizomatic Server

Rust service for storing and querying rhizomatic themagraphs outside the Obsidian vault.

It provides:

- JSON CRUD endpoints for themagraphs and their intralinks
- Rhizomatic query evaluation with `[[link]]`, `&&`, `||`, `!`, `*[[expansion]]`, and parentheses
- A lightweight web UI for creating, editing, browsing, deleting, and searching themagraphs
- SQLite persistence so the dataset can live independently from vault markdown files

## Why this exists

This project is intended to complement the existing themagraph ecosystem already living under `programming` and Gitea:

- Gitea: `https://gitea.chiliahedron.wtf/john-craig/rhizoidlib`
- Local repo: `/home/evak/programming/by_category/note_taking/rhizoidlib`
- Local repo: `/home/evak/programming/by_category/knowledge_management/vault-mcp-server`
- Local repo: `/home/evak/programming/by_category/knowledge_management/vault-ingestion-pipeline`
- Local repo: `/home/evak/programming/by_category/note_taking/rhizomatic-notes`

Those projects currently work primarily against the Obsidian vault at `/home/evak/documents/by_category/vault`. This server gives you a database-backed CRUD and query surface that can sit beside that vault-oriented workflow, while keeping the query semantics close to `rhizoidlib`.

## Data model

Each themagraph stores:

- `id`: UUID
- `body`: freeform themagraph text
- `links`: normalized intralinks, combining explicit links from the API/UI and wikilinks found in the body
- `created_at`
- `updated_at`

## Running

```sh
cargo run
```

Defaults:

- `BIND_ADDRESS=127.0.0.1:3000`
- `DATABASE_URL=sqlite:rhizomatic-server.db?mode=rwc`

Then open `http://127.0.0.1:3000`.

## API

### Create

```sh
curl -X POST http://127.0.0.1:3000/api/themagraphs \
  -H 'content-type: application/json' \
  -d '{
    "body": "Building [[programming rhizomatic server]] for [[rhizomatic]] themagraph CRUD.",
    "links": ["programming rhizomatic server", "rhizomatic"]
  }'
```

### List

```sh
curl http://127.0.0.1:3000/api/themagraphs
```

### Query

```sh
curl 'http://127.0.0.1:3000/api/query?query=%5B%5Bprogramming%20rhizomatic%20server%5D%5D%20%26%26%20%5B%5Brhizomatic%5D%5D'
```

## Notes on compatibility

- Query parsing is intentionally modeled after the current `rhizoidlib` implementation instead of inventing a separate syntax.
- Link normalization follows the same main rules as `rhizoidlib`: strip wrapping `[[...]]`, ignore aliases after `|`, and ignore headings after `#`.
- This service does not yet parse or emit pilcrow-delimited markdown payloads. It stores normalized themagraph records directly in SQLite.
