# Hosted-runner PoC. Never read a locally installed application/profile.
[CmdletBinding()]
param([ValidateSet('bridge-roundtrip', 'native-client', 'workspace-link')][string]$Scenario = 'bridge-roundtrip')
$ErrorActionPreference = 'Stop'
if ($env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows') { throw 'Requires a disposable hosted Windows runner' }
$report = Join-Path $env:RUNNER_TEMP 'native-smoke-report'
$capture = Join-Path $env:RUNNER_TEMP ('native-smoke-' + [guid]::NewGuid())
$client = Join-Path $capture 'client'
$bridge = Join-Path $capture 'bridge'
$exe = (Resolve-Path './target/debug/kaminide-gpui.exe').Path
New-Item -ItemType Directory -Path $report,$capture | Out-Null
$runner = $null
$app = $null
try {
    # Launcher requires a registry stub next to the exe when the host is enabled.
    'fn main() { std::process::exit(1); }' | rustc - --crate-name reg_stub -o ./target/debug/reg.exe
    if ($LASTEXITCODE) { throw 'Registry stub compilation failed' }
    $env:KAMIN_PROBE_PORT = '19357'
    $env:KAMIN_PROBE_TOKEN = [guid]::NewGuid().ToString('N')
    if ($Scenario -ne 'native-client') {
        $env:CLAUDE_PROXY_PORT = '13456'
        $env:BRIDGE_CODEX_BIN = (Resolve-Path 'extensions/claude-bridge/server/test-support/fake-codex.mjs').Path
        $env:FAKE_CODEX_LOGGED_IN = '1'
        $runner = Start-Process node -ArgumentList @('extensions/claude-bridge/server/test-support/run.mjs', ("`"$bridge`"")) -PassThru -WindowStyle Hidden
    }
    ./scripts/start_native_acceptance.ps1 -Exe $exe -EvidenceDir $client -WithHost -Warp -SoftwareCef -Port 19357
    $manifest = Get-Content -Raw (Join-Path $client 'manifest.json') | ConvertFrom-Json
    $app = Get-Process -Id $manifest.pid -ErrorAction SilentlyContinue
    node (Join-Path $PSScriptRoot 'probe-smoke.mjs') $Scenario $client $report
    if ($LASTEXITCODE) { throw 'Native scenario failed. See report.json.' }
} finally {
    foreach ($child in @($app, $runner)) {
        if ($child -and !$child.HasExited) { taskkill /PID $child.Id /T /F | Out-Null }
    }
    # Never archive server state, probe tokens, logs or the browser profile.
    Remove-Item -LiteralPath $capture -Recurse -Force
}
