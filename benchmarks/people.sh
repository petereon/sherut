#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DB_PATH="${SCRIPT_DIR}/data.db"

result=$(sqlite3 "$DB_PATH" -json "SELECT * FROM people WHERE id=$1;" | jq -r ".[0]")

if [ "$result" = "null" ] || [ -z "$result" ]; then
  echo "@status: 404"
  echo "{\"error\": \"Person with id $1 not found\"}"
  exit 0
fi

echo "$result"