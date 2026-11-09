#!/bin/sh
set -eu
# Acceptance feeds the credential over an attached stdin pipe. It never appears
# in Podman's argv, configured environment, a host file or a diagnostic log.
if [ "${1:-}" = '--token-stdin' ]; then
  IFS= read -r BRIDGE_TOKEN
  export BRIDGE_TOKEN
  shift
fi
if [ -n "${BRIDGE_TOKEN:-}" ] && [ -n "${BRIDGE_TOKEN_FILE:-}" ]; then
  echo 'Configure only one of BRIDGE_TOKEN and BRIDGE_TOKEN_FILE.' >&2
  exit 64
fi
if [ -z "${BRIDGE_TOKEN:-}" ] && [ -z "${BRIDGE_TOKEN_FILE:-}" ]; then
  echo 'BRIDGE_TOKEN or BRIDGE_TOKEN_FILE is required.' >&2
  exit 64
fi
if [ -n "${BRIDGE_TOKEN_FILE:-}" ]; then
  BRIDGE_TOKEN=$(cat -- "$BRIDGE_TOKEN_FILE")
  export BRIDGE_TOKEN
  unset BRIDGE_TOKEN_FILE
fi
if [ -z "${BRIDGE_TOKEN:-}" ] || [ -z "${BRIDGE_SERVER_URL:-}" ] || [ -z "${WORKSPACE_ORIGIN:-}" ]; then
  echo 'A nonempty token, BRIDGE_SERVER_URL and exact WORKSPACE_ORIGIN are required.' >&2
  exit 64
fi
if [ ! -f dist/cli.cjs ]; then
  echo 'Workspace executable dist/cli.cjs is not built: core composition is pending (INC-2026-0087).' >&2
  exit 78
fi
exec node dist/cli.cjs "$@"
