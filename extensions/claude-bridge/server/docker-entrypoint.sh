#!/bin/bash
set -e

# Ensure Claude Code onboarding is marked complete (skip theme/login prompts)
CLAUDE_JSON="$HOME/.claude.json"
if [ ! -f "$CLAUDE_JSON" ]; then
  echo '{}' > "$CLAUDE_JSON"
fi
node <<'ONBOARDING'
const fs = require('fs');
const p = process.env.HOME + '/.claude.json';
const j = JSON.parse(fs.readFileSync(p, 'utf8'));
let changed = false;
if (!j.hasCompletedOnboarding) { j.hasCompletedOnboarding = true; changed = true; }
if (!j.theme) { j.theme = 'dark'; changed = true; }
if (!j.hasTrustDialogAccepted) { j.hasTrustDialogAccepted = true; changed = true; }
if (!j.hasCompletedProjectOnboarding) { j.hasCompletedProjectOnboarding = true; changed = true; }
if (!j.projects) j.projects = {};
const trustPaths = ['/app', '/home/bridge', process.env.HOME || '/home/bridge'];
for (const tp of trustPaths) {
  if (!j.projects[tp]) j.projects[tp] = {};
  if (!j.projects[tp].hasTrustDialogAccepted) { j.projects[tp].hasTrustDialogAccepted = true; changed = true; }
}
if (changed) { fs.writeFileSync(p, JSON.stringify(j, null, 2)); console.log('[entrypoint] Claude onboarding flags set'); }
ONBOARDING

# Ensure bypass permissions warning is suppressed
CLAUDE_SETTINGS="$HOME/.claude/settings.json"
mkdir -p "$HOME/.claude"
if [ ! -f "$CLAUDE_SETTINGS" ]; then
  echo '{}' > "$CLAUDE_SETTINGS"
fi
node <<'SETTINGS'
const fs = require('fs');
const p = (process.env.HOME || '/home/bridge') + '/.claude/settings.json';
const j = JSON.parse(fs.readFileSync(p, 'utf8'));
if (!j.skipDangerousModePermissionPrompt) {
  j.skipDangerousModePermissionPrompt = true;
  fs.writeFileSync(p, JSON.stringify(j, null, 2));
  console.log('[entrypoint] Bypass permissions prompt suppressed');
}
SETTINGS

# Explicit V8 heap budget for the server, derived from the memory this
# container may use (cgroup limit, else host RAM): 40%, within 1–8 GiB. The rest
# stays for Buffers, the DuckDB budget (src/core/stats/database/memory-budget.ts)
# and the Claude CLI children. An operator-provided --max-old-space-size wins.
if [[ "${NODE_OPTIONS:-}" != *max-old-space-size* ]]; then
  # kB from /proc/meminfo, scaled in bash (64-bit): mawk's printf %d would
  # clamp a byte count above 2 GiB.
  total_kb=$(awk '/^MemTotal:/ { print $2 }' /proc/meminfo 2>/dev/null)
  [[ "$total_kb" =~ ^[0-9]+$ ]] || total_kb=0
  total_bytes=$(( total_kb * 1024 ))
  limit_bytes=$(cat /sys/fs/cgroup/memory.max 2>/dev/null || cat /sys/fs/cgroup/memory/memory.limit_in_bytes 2>/dev/null || echo max)
  if [[ ! "$limit_bytes" =~ ^[0-9]+$ ]] || { [[ "$total_bytes" -gt 0 ]] && [[ "$limit_bytes" -ge "$total_bytes" ]]; }; then
    limit_bytes=$total_bytes
  fi
  heap_mb=$(( limit_bytes / 1048576 * 40 / 100 ))
  if [[ "$heap_mb" -lt 1024 ]]; then heap_mb=1024; fi
  if [[ "$heap_mb" -gt 8192 ]]; then heap_mb=8192; fi
  export NODE_OPTIONS="${NODE_OPTIONS:+$NODE_OPTIONS }--max-old-space-size=${heap_mb}"
  echo "[entrypoint] V8 heap budget: ${heap_mb} MiB"
fi

# Execute the CMD passed by Docker
exec -- "$@"
