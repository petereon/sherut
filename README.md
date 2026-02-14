# Sherut

The tool you didn't know you wanted. A slow breath of fresh air in the world of ever faster API frameworks.

Sherut (שירות - Hebrew for "service") is a lightweight tool that exposes shell commands as HTTP endpoints. It automatically detects your shell, passes request data to your scripts, and even auto-detects content types.

> [!WARNING]
> Please keep in mind `sherut` is untested and meant as an elaborate joke more than anything else. Production use is very much discouraged.

## Installation

```bash
cargo build --release
```

The binary will be at `target/release/sherut`.

## Quick Start

```bash
# Simple echo endpoint
sherut --route "/hello/:name" 'echo "Hello, :name!"'

# JSON API using a script
sherut --route "/api/users/:id" './scripts/get_user.sh :id'

# Multiple routes
sherut \
  --route "/users" 'sqlite3 data.db -json "SELECT * FROM users"' \
  --route "/users/:id" 'sqlite3 data.db -json "SELECT * FROM users WHERE id=:id" | jq ".[0]"'
```

## Features

### Route Parameters

Use `:param` syntax in routes. The same `:param` placeholders can be used in commands:

```bash
sherut --route "/users/:id/posts/:postId" 'echo "User :id, Post :postId"'
```

### Query String Parameters

Access query parameters via the `QUERY` associative array (bash/zsh) or `QUERY_JSON` environment variable:

```bash
# GET /search?q=hello&limit=10

# Bash/Zsh (assoc array)
sherut --route "/search" 'echo "Query: ${QUERY[q]}, Limit: ${QUERY[limit]}"'

# Any shell (JSON)
sherut --query-format json --route "/search" 'echo $QUERY_JSON | jq -r .q'
```

### HTTP Headers

Access request headers via the `HEADERS` associative array or `HEADERS_JSON` environment variable:

```bash
# Bash/Zsh
sherut --route "/auth" 'echo "Token: ${HEADERS[authorization]}"'

# JSON format
sherut --header-format json --route "/auth" 'echo $HEADERS_JSON | jq -r .authorization'
```

### Response Control

Control HTTP responses using magic prefixes in your script output:

```bash
#!/bin/bash
# Set custom status code
echo "@status: 201"

# Set custom headers
echo "@header: X-Custom-Header: my-value"
echo "@header: Cache-Control: no-cache"

# Response body
echo '{"created": true}'
```

### Auto Content-Type Detection

Sherut automatically detects and sets the `Content-Type` header:

- **JSON**: `application/json` (when output is valid JSON starting with `{` or `[`)
- **XML**: `application/xml` (when output starts with `<?xml` or looks like XML)
- **HTML**: `text/html` (when output starts with `<!doctype html` or `<html`)
- **Default**: `text/plain`

You can override this with `@header: Content-Type: ...`.

## CLI Options

| Option | Default | Description |
|--------|---------|-------------|
| `--port` | `8080` | Port to listen on |
| `--log-level` | `info` | Log level: `error`, `warn`, `info`, `debug`, `trace` |
| `--shell` | auto | Shell to use: `bash`, `zsh`, `fish`, `sh` (auto-detected from `$SHELL`) |
| `--header-format` | auto | How to pass headers: `assoc` (associative array) or `json` |
| `--query-format` | auto | How to pass query params: `assoc` or `json` |
| `--route PATH CMD` | - | Define a route (can be repeated) |

## Examples

### SQLite REST API

```bash
#!/bin/bash
# examples/bin/people

result=$(sqlite3 data.db -json "SELECT * FROM people WHERE id=$1;" | jq -r ".[0]")

if [ "$result" = "null" ] || [ -z "$result" ]; then
  echo "@status: 404"
  echo '{"error": "Not found"}'
  exit 0
fi

echo "$result"
```

```bash
env PATH="$(pwd)/examples/bin:$PATH" sherut --port 8080 --route "/people/:id" './examples/bin/people :id'
```

```bash
➜  ~ curl http://0.0.0.0:8080/people/1
{
  "id": 1,
  "name": "Alice",
  "age": 30
}
```

### Search with Query Parameters

```bash
sherut --port 8080 --route "/search" '
  q="${QUERY[q]}"
  limit="${QUERY[limit]:-10}"
  sqlite3 data.db -json "SELECT * FROM items WHERE name LIKE \"%$q%\" LIMIT $limit"
'
```

```bash
➜  ~ curl 'http://0.0.0.0:8080/search?q=apple'
[{"id":1,"name":"Apple","price":1.5},
{"id":4,"name":"Pineapple","price":3.5}]
```


### Authenticated Endpoint

```bash
sherut --route "/admin/stats" '
  token="${HEADERS[authorization]}"
  if [ "$token" != "Bearer secret123" ]; then
    echo "@status: 401"
    echo "Unauthorized"
    exit 0
  fi
  echo "Secret stats here"
'
```

## Shell Support

| Shell | Associative Arrays | Notes |
|-------|-------------------|-------|
| bash | ✅ `HEADERS`, `QUERY` | Full support |
| zsh | ✅ `HEADERS`, `QUERY` | Full support |
| fish | ❌ Use JSON format | Use `--header-format json --query-format json` |
| sh | ❌ Use JSON format | Use `--header-format json --query-format json` |

## License

MIT
