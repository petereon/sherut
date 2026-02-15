# Sherut

The tool you didn't know you wanted. A slow breath of fresh air in the world of ever faster API frameworks.

Sherut (שירות - Hebrew for "service") is a lightweight tool that exposes shell commands as HTTP endpoints. It automatically detects your shell, passes request data to your scripts, and even auto-detects content types.

> [!WARNING]
> Please keep in mind `sherut` meant as an elaborate joke more than anything else. Production use is very much discouraged.

## Installation

```bash
cargo build --release
```

The binary will be at `target/release/sherut`.

## Quick Start

```bash
# Simple echo endpoint (matches any HTTP method)
sherut --route "/hello/:name" 'echo "Hello, :name!"'

# Specify HTTP methods
sherut --route "GET /api/users/:id" './scripts/get_user.sh :id'

# Multiple routes with different methods
sherut \
  --route "GET /users" 'sqlite3 data.db -json "SELECT * FROM users"' \
  --route "GET /users/:id" 'sqlite3 data.db -json "SELECT * FROM users WHERE id=:id" | jq ".[0]"' \
  --route "POST /users" './scripts/create_user.sh'
```

## Features

### HTTP Methods

Specify the HTTP method before the path. Supported methods: `GET`, `POST`, `PUT`, `DELETE`, `PATCH`, `HEAD`, `OPTIONS`, or `ANY` (default):

```bash
# Only handles GET requests
sherut --route "GET /users" 'sqlite3 data.db -json "SELECT * FROM users"'

# Only handles POST requests  
sherut --route "POST /users" 'echo "@status: 201"; echo "{\"created\": true}"'

# Handles any HTTP method (default when method not specified)
sherut --route "/health" 'echo "OK"'
sherut --route "ANY /health" 'echo "OK"'  # equivalent
```

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

### Request Body

The request body is passed to your command via **stdin**, following Unix conventions:

```bash
# Echo the raw body back
sherut --route "POST /echo" 'cat'

# Parse JSON body with jq
sherut --route "POST /users" 'jq -r .name | xargs -I{} echo "Hello, {}!"'

# Process and store
sherut --route "POST /data" 'cat > /tmp/received.json && echo "Saved!"'

# Validate JSON and extract fields
sherut --route "POST /api/items" '
  name=$(jq -r .name)
  price=$(jq -r .price)
  sqlite3 data.db "INSERT INTO items (name, price) VALUES (\"$name\", $price)"
  echo "{\"created\": true}"
'
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
| `--route PATH CMD` | - | Define a route. PATH can include HTTP method (e.g., "GET /users") |

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


## Benchmark Result
```
========================================
  Sherut vs FastAPI Benchmark
========================================
Cleaning up...

=== Benchmarking FastAPI ===

FastAPI server ready on http://localhost:8080

Benchmark 1: curl -s http://localhost:8080/people/1
  Time (mean ± σ):       9.9 ms ±   0.6 ms    [User: 3.1 ms, System: 3.9 ms]
  Range (min … max):     8.7 ms …  12.3 ms    100 runs
 

FastAPI benchmark complete
Cleaning up...

=== Benchmarking Sherut ===

Building sherut...
2026-02-15T10:12:57.099832Z  INFO sherut: Using shell: zsh
2026-02-15T10:12:57.099872Z  INFO sherut: Header format: Assoc
2026-02-15T10:12:57.099873Z  INFO sherut: Query format: Assoc
2026-02-15T10:12:57.100304Z  INFO sherut::routes: Registered route: GET /people/:id -> `./benchmarks/people.sh :id`
2026-02-15T10:12:57.100399Z  INFO sherut: 🚀 Server running on http://0.0.0.0:8080
Sherut server ready on http://localhost:8080

Benchmark 1: curl -s http://localhost:8080/people/1
  Time (mean ± σ):      27.6 ms ±   1.9 ms    [User: 3.1 ms, System: 3.8 ms]
  Range (min … max):    25.4 ms …  42.8 ms    100 runs
 
  Warning: Statistical outliers were detected. Consider re-running this benchmark on a quiet system without any interferences from other programs. It might help to use the '--warmup' or '--prepare' options.
 

Sherut benchmark complete
Cleaning up...

=== Comparison ===

FastAPI: 9.93ms (mean)
Sherut:  27.60ms (mean)

FastAPI is 2.78x faster than Sherut

Benchmark complete!
Cleaning up...
```

## License

MIT
