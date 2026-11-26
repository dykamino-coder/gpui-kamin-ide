[CmdletBinding()]
param([Parameter(Mandatory)][string]$DataDirectory)
$ErrorActionPreference = 'Stop'
$env:CLAUDE_PROXY_PORT = '13456'
$env:BRIDGE_CODEX_BIN = (Resolve-Path 'extensions/claude-bridge/server/test-support/fake-codex.mjs').Path
$env:FAKE_CODEX_LOGGED_IN = '1'
Start-Process node -ArgumentList @('extensions/claude-bridge/server/test-support/run.mjs', ("`"$DataDirectory`"")) -PassThru -WindowStyle Hidden
