# Hosted-runner PoC. Never read a locally installed application/profile.
[CmdletBinding()]
param([ValidateSet('bridge-roundtrip', 'native-client', 'workspace-link', 'console-scroll', 'engine-menus')][string]$Scenario = 'bridge-roundtrip')
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
$stage = 'initialize'
try {
    # Launcher requires a registry stub next to the exe when the host is enabled.
    'fn main() { std::process::exit(1); }' | rustc - --crate-name reg_stub -o ./target/debug/reg.exe
    if ($LASTEXITCODE) { throw 'Registry stub compilation failed' }
    $env:KAMIN_PROBE_PORT = '19357'
    $env:KAMIN_PROBE_TOKEN = [guid]::NewGuid().ToString('N')
    if ($Scenario -ne 'native-client') {
        $runner = & (Join-Path $PSScriptRoot 'start-bridge.ps1') -DataDirectory $bridge
    }
    # Immutable product candidates can predate the launcher fix for the default
    # adapter policy required by DebugDevice::Warp.
    $env:KAMIN_GPU_ADAPTER = 'default'
    $stage = 'launch-client'
    ./scripts/start_native_acceptance.ps1 -Exe $exe -EvidenceDir $client -WithHost -Warp -SoftwareCef -Port 19357
    $manifest = Get-Content -Raw (Join-Path $client 'manifest.json') | ConvertFrom-Json
    $app = Get-Process -Id $manifest.pid -ErrorAction SilentlyContinue
    $stage = 'exercise-client'
    node (Join-Path $PSScriptRoot 'probe-smoke.mjs') $Scenario $client $report
    if ($LASTEXITCODE) { throw 'Native scenario failed. See report.json.' }
    $stage = 'complete'
} finally {
    $bin = Split-Path -Parent $exe
    $runtime = [ordered]@{
        stage = $stage
        session = (Get-Process -Id $PID).SessionId
        interactive = [Environment]::UserInteractive
        cefDll = Test-Path -LiteralPath (Join-Path $bin 'libcef.dll')
        cefHelper = Test-Path -LiteralPath (Join-Path $bin 'kaminide-web.exe')
        clientPidObserved = [bool]$app
        clientExited = if ($app) { $app.HasExited } else { $null }
        clientExit = if ($app -and $app.HasExited) { $app.ExitCode } else { $null }
        bridgeExited = if ($runner) { $runner.HasExited } else { $null }
        profileRemoved = $false
    }
    try {
        & (Join-Path $PSScriptRoot 'remove-profile.ps1') -Directory $capture -Processes @($app,$runner)
        $runtime.profileRemoved = !(Test-Path -LiteralPath $capture)
    } finally {
        $runtime | ConvertTo-Json | Set-Content (Join-Path $report 'runtime.json')
    }
}

# A child may exit between enumeration and taskkill; all owned state has been removed.
exit 0
