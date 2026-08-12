#!/usr/bin/env bash
set -euo pipefail

BRIDGE_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
PROJECT_DIR="$(CDPATH= cd -- "$BRIDGE_DIR/../.." && pwd)"

if [[ "${1:-}" == "--install" ]]; then
  npm --prefix "$PROJECT_DIR" ci --ignore-scripts
  npm --prefix "$BRIDGE_DIR/server" ci --ignore-scripts
  npm --prefix "$BRIDGE_DIR/extension" ci --ignore-scripts
  npm --prefix "$BRIDGE_DIR/webview" ci --ignore-scripts
elif [[ $# -gt 0 ]]; then
  echo "usage: $0 [--install]" >&2
  exit 2
fi

npm --prefix "$PROJECT_DIR" run typecheck
npm --prefix "$PROJECT_DIR" run lint
npm --prefix "$PROJECT_DIR" test

npm --prefix "$BRIDGE_DIR/server" run typecheck
npm --prefix "$BRIDGE_DIR/server" test
npm --prefix "$BRIDGE_DIR/extension" run typecheck
npm --prefix "$BRIDGE_DIR/extension" test
npm --prefix "$BRIDGE_DIR/webview" run typecheck
npm --prefix "$BRIDGE_DIR/webview" test

npm --prefix "$BRIDGE_DIR/extension" run build
npm --prefix "$BRIDGE_DIR/webview" run build

if rg -n "from ['\"]electron['\"]|import\\(['\"]electron['\"]\\)|require\\(['\"]electron['\"]\\)" \
  "$BRIDGE_DIR/extension/src" \
  "$PROJECT_DIR/builtin-extensions/claude-bridge/extension.js"; then
  echo "current Bridge runtime must import @kaminide/host-compat, not electron" >&2
  exit 1
fi

git -C "$PROJECT_DIR" diff --exit-code -- \
  builtin-extensions/claude-bridge/extension.js \
  builtin-extensions/claude-bridge/chat.html \
  builtin-extensions/claude-bridge/customize.html \
  builtin-extensions/claude-bridge/tools.html
