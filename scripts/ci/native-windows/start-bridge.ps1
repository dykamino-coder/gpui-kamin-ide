[CmdletBinding()]
param([Parameter(Mandatory)][string]$DataDirectory)
$ErrorActionPreference = 'Stop'
$env:CLAUDE_PROXY_PORT = '13456'
$env:BRIDGE_CODEX_BIN = (Resolve-Path 'extensions/claude-bridge/server/test-support/fake-codex.mjs').Path
# The real app-server launcher correctly strips test-only environment variables.
# Seed only the fake provider's account fixture inside this throwaway Bridge home.
$env:BRIDGE_CODEX_HOME = Join-Path $DataDirectory 'codex-home'
$state = Join-Path $env:BRIDGE_CODEX_HOME 'fake-state'
New-Item -ItemType Directory -Path $state -Force | Out-Null
@{ email = 'runner@example.test'; planType = 'plus' } | ConvertTo-Json -Compress |
    Set-Content -Encoding utf8NoBOM -LiteralPath (Join-Path $state 'account.json')
Start-Process node -ArgumentList @('extensions/claude-bridge/server/test-support/run.mjs', ("`"$DataDirectory`"")) -PassThru -WindowStyle Hidden
