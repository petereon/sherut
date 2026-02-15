# shellcheck shell=sh

# Defining variables and functions here will affect all specfiles.
# Change shell options inside a function may cause different behavior,
# so it is better to set them here.
# set -eu

# Server configuration
SHERUT_PORT="${SHERUT_PORT:-8080}"
SHERUT_HOST="${SHERUT_HOST:-localhost}"
SHERUT_BASE_URL="http://${SHERUT_HOST}:${SHERUT_PORT}"
SHERUT_PID_FILE="/tmp/sherut_test.pid"
SHERUT_LOG_FILE="/tmp/sherut_test.log"

# Start sherut server with given routes
# Usage: start_sherut --route "GET /hello" "echo hello" --route "/api" "cat"
start_sherut() {
  # Build first to avoid timeout during compilation
  cargo build --quiet 2>/dev/null || cargo build

  # Start server in background
  cargo run --quiet -- --port "$SHERUT_PORT" "$@" > "$SHERUT_LOG_FILE" 2>&1 &
  echo $! > "$SHERUT_PID_FILE"

  # Wait for server to be ready
  wait_for_sherut
}

# Wait for sherut to be ready (max 10 seconds)
wait_for_sherut() {
  local max_attempts=50
  local attempt=0

  while [ $attempt -lt $max_attempts ]; do
    if curl -s -o /dev/null -w "" "http://${SHERUT_HOST}:${SHERUT_PORT}/" 2>/dev/null; then
      return 0
    fi
    # Also check if we get a 404, which means server is up
    if curl -s -o /dev/null -w "%{http_code}" "http://${SHERUT_HOST}:${SHERUT_PORT}/" 2>/dev/null | grep -q "404"; then
      return 0
    fi
    sleep 0.2
    attempt=$((attempt + 1))
  done

  echo "Sherut failed to start. Log contents:" >&2
  cat "$SHERUT_LOG_FILE" >&2
  return 1
}

# Stop sherut server
stop_sherut() {
  if [ -f "$SHERUT_PID_FILE" ]; then
    local pid
    pid=$(cat "$SHERUT_PID_FILE")
    if [ -n "$pid" ]; then
      kill "$pid" 2>/dev/null || true
      # Also kill any cargo run children
      pkill -P "$pid" 2>/dev/null || true
      # Wait briefly for process to terminate
      sleep 0.3
    fi
    rm -f "$SHERUT_PID_FILE"
  fi
  # Cleanup any remaining sherut processes on this port
  pkill -f "sherut.*--port.*$SHERUT_PORT" 2>/dev/null || true
  return 0
}

# HTTP request helpers
http_get() {
  curl -s "$SHERUT_BASE_URL$1"
}

http_get_status() {
  curl -s -o /dev/null -w "%{http_code}" "$SHERUT_BASE_URL$1"
}

http_post() {
  local path="$1"
  local data="${2:-}"
  curl -s -X POST -d "$data" "$SHERUT_BASE_URL$path"
}

http_post_json() {
  local path="$1"
  local data="${2:-}"
  curl -s -X POST -H "Content-Type: application/json" -d "$data" "$SHERUT_BASE_URL$path"
}

http_put() {
  local path="$1"
  local data="${2:-}"
  curl -s -X PUT -d "$data" "$SHERUT_BASE_URL$path"
}

http_delete() {
  curl -s -X DELETE "$SHERUT_BASE_URL$1"
}

http_response_header() {
  local path="$1"
  local header="$2"
  curl -s -I "$SHERUT_BASE_URL$path" | grep -i "^$header:" | cut -d: -f2- | tr -d '\r' | xargs
}

# This callback function will be invoked only once before loading specfiles.
spec_helper_precheck() {
  # Available functions: info, warn, error, abort, setenv, unsetenv
  : minimum_version "0.28.1"
}

# This callback function will be invoked after a specfile has been loaded.
spec_helper_loaded() {
  :
}

# This callback function will be invoked after core modules has been loaded.
spec_helper_configure() {
  # Available functions: import, before_each, after_each, before_all, after_all
  : import 'support/custom_matcher'
}
