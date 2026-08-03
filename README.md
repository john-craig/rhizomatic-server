# Rhizomatic Server

Rust service for storing and querying rhizomatic themagraphs outside the Obsidian vault.

It provides:

- JSON CRUD endpoints for themagraphs and their intralinks
- Rhizomatic query evaluation with `[[link]]`, `&&`, `||`, `!`, `*[[expansion]]`, and parentheses
- Named-query expansion for one-link themagraphs whose bodies are valid rhizomatic queries
- A tabbed web UI with query-driven Search, themagraph Create, and fuzzy-searchable Links views
- Search can save a valid query as a named-query themagraph, and Links can be constrained to named queries
- SQLite persistence so the dataset can live independently from vault markdown files
- API-token authentication for the JSON API, configured from a server-side config file
- A stdio-based MCP server executable that exposes the HTTP API as MCP tools

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
cargo run -- --config ./config.toml
```

Create `config.toml` from the example:

```sh
cp config.example.toml config.toml
```

Config file fields:

- `bind_address`
- `database_url`
- `api_token`

Then open `http://127.0.0.1:3000`.

The web UI routes are intentionally unauthenticated. Only `/api/*` requires a token.

## MCP Server

This project also provides a second executable:

```sh
cargo run --bin rhizomatic-mcp-server -- --server-url http://127.0.0.1:3000
```

It runs as an stdio MCP server and exposes these tools:

- `rhizomatic_api_health`
- `list_themagraphs`
- `get_themagraph`
- `get_themagraph_by_uuid`
- `create_themagraph`
- `update_themagraph`
- `modify_themagraph`
- `delete_themagraph`
- `query_themagraphs`
- `query_themagraphs_rhizomatic`
- `query_themagraphs_regex`
- `list_links`
- `list_named_query_links`
- `list_tags`
- `query_tags_regex`
- `create_tag`

Each tool accepts an `api_token_file` parameter. The file should contain the same API token configured in `config.toml`.

Example token file:

```sh
printf '%s\n' 'replace-with-a-long-random-token' > /tmp/rhizomatic-server.token
```

## API

Supply the token either as `Authorization: Bearer <token>` or `X-API-Token: <token>`.

### Create

```sh
curl -X POST http://127.0.0.1:3000/api/themagraphs \
  -H 'authorization: Bearer replace-with-a-long-random-token' \
  -H 'content-type: application/json' \
  -d '{
    "body": "Building [[programming rhizomatic server]] for [[rhizomatic]] themagraph CRUD.",
    "links": ["programming rhizomatic server", "rhizomatic"]
  }'
```

### List

```sh
curl \
  -H 'authorization: Bearer replace-with-a-long-random-token' \
  http://127.0.0.1:3000/api/themagraphs
```

### Query

```sh
curl \
  -H 'authorization: Bearer replace-with-a-long-random-token' \
  'http://127.0.0.1:3000/api/query?query=%5B%5Bprogramming%20rhizomatic%20server%5D%5D%20%26%26%20%5B%5Brhizomatic%5D%5D'
```

### Regex Query

```sh
curl -X POST http://127.0.0.1:3000/api/query/regex \
  -H 'authorization: Bearer replace-with-a-long-random-token' \
  -H 'content-type: application/json' \
  -d '{
    "pattern": "rhizomatic",
    "case_insensitive": true,
    "target": "any"
  }'
```

### Tags

List tags:

```sh
curl \
  -H 'authorization: Bearer replace-with-a-long-random-token' \
  http://127.0.0.1:3000/api/tags
```

Create a tag:

```sh
curl -X POST http://127.0.0.1:3000/api/tags \
  -H 'authorization: Bearer replace-with-a-long-random-token' \
  -H 'content-type: application/json' \
  -d '{"name":"programming rhizomatic server"}'
```

Regex-filter tags:

```sh
curl -X POST http://127.0.0.1:3000/api/tags/query/regex \
  -H 'authorization: Bearer replace-with-a-long-random-token' \
  -H 'content-type: application/json' \
  -d '{
    "pattern": "^programming",
    "case_insensitive": true
  }'
```

### Intralinks

List every intralink, including whether it is a named query:

```sh
curl \
  -H 'authorization: Bearer replace-with-a-long-random-token' \
  http://127.0.0.1:3000/api/links
```

List only named-query intralinks:

```sh
curl \
  -H 'authorization: Bearer replace-with-a-long-random-token' \
  http://127.0.0.1:3000/api/links/named-queries
```

## Notes on compatibility

- Query parsing is intentionally modeled after the current `rhizoidlib` implementation instead of inventing a separate syntax.
- Link normalization follows the same main rules as `rhizoidlib`: strip wrapping `[[...]]`, ignore aliases after `|`, and ignore headings after `#`.
- This service does not yet parse or emit pilcrow-delimited markdown payloads. It stores normalized themagraph records directly in SQLite.
