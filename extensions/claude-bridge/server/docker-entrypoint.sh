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

# Execute the CMD passed by Docker
exec -- "$@"
