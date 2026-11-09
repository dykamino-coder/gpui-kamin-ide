#!/bin/sh
set -eu
# Acceptance feeds the credential over an attached stdin pipe. It never appears
# in Podman's argv, configured environment, a host file or a diagnostic log.
if [ "${1:-}" = '--token-stdin' ]; then
  IFS= read -r BRIDGE_TOKEN
  export BRIDGE_TOKEN
  shift
fi
if [ "${1:-}" != '--reset-credentials' ] && { [ -z "${BRIDGE_SERVER_URL:-}" ] || [ -z "${WORKSPACE_ORIGIN:-}" ]; }; then
  echo 'BRIDGE_SERVER_URL and exact WORKSPACE_ORIGIN are required.' >&2
  exit 64
fi
if [ ! -f dist/cli.cjs ]; then
  echo 'Workspace executable dist/cli.cjs is not built: core composition is pending (INC-2026-0087).' >&2
  exit 78
fi
exec node dist/cli.cjs "$@"
