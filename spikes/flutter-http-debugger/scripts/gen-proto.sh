#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

PROTO_ROOT="../../proto"
PROTO_FILE="$PROTO_ROOT/membench/dashboard/v1/debugger.proto"
OUT_DIR="lib/src/gen"

if ! command -v protoc >/dev/null 2>&1; then
  echo "protoc is required" >&2
  exit 1
fi

if ! command -v protoc-gen-dart >/dev/null 2>&1; then
  echo "protoc-gen-dart is required. Install with:" >&2
  echo "  dart pub global activate protoc_plugin" >&2
  echo "and add ~/.pub-cache/bin to PATH." >&2
  exit 1
fi

mkdir -p "$OUT_DIR"
protoc --dart_out="$OUT_DIR" -I "$PROTO_ROOT" "$PROTO_FILE"
dart format "$OUT_DIR/membench/dashboard/v1/debugger.pb.dart" \
  "$OUT_DIR/membench/dashboard/v1/debugger.pbjson.dart" \
  "$OUT_DIR/membench/dashboard/v1/debugger.pbenum.dart"
