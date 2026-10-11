# Launch a disposable DEBUG candidate with application-level acceptance seams.
# No system-wide DirectX/graphics setting is read or changed: WARP and device
# loss are requested from the debug build itself through its environment.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$EvidenceDir,
    [int]$Port = 9357,
    [switch]$Warp,
    [switch]$DeviceLoss,
    [switch]$ReducedAnimations,
    [switch]$SoftwareCef,
    [switch]$WithHost,
    [switch]$Wait
)
$ErrorActionPreference = 'Stop'
if ($Warp -and $DeviceLoss) { throw '-Warp and -DeviceLoss select different GPUI devices; run them separately.' }
$candidate = (Resolve-Path -LiteralPath $Exe).Path
$bytes = [IO.File]::ReadAllBytes($candidate)
if (![Text.Encoding]::UTF8.GetString($bytes).Contains('debug-native-acceptance-v1')) {
    throw 'A debug native-acceptance candidate is required; release was not launched.'
}
$repo = Split-Path -Parent $PSScriptRoot
$evidence = [IO.Path]::GetFullPath($EvidenceDir)
foreach ($line in (git -C $repo worktree list --porcelain)) {
    if ($line.StartsWith('worktree ')) {
        $worktree = [IO.Path]::GetFullPath($line.Substring(9))
        if ($evidence -eq $worktree -or $evidence.StartsWith($worktree + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'EvidenceDir must be outside every public worktree.'
        }
    }
}
if (Test-Path -LiteralPath $evidence) { throw 'Use a new capture directory; existing evidence is preserved.' }
$probeSocket = [Net.Sockets.TcpClient]::new()
try {
    $connected = $probeSocket.ConnectAsync('127.0.0.1', $Port)
    if ($connected.Wait(500) -and $probeSocket.Connected) { throw 'Probe port is occupied; choose another port.' }
} catch [AggregateException] {
    # Connection refused means the requested isolated port is free.
} finally { $probeSocket.Dispose() }
if ($WithHost -and !(Test-Path -LiteralPath (Join-Path (Split-Path -Parent $candidate) 'reg.exe'))) {
    # A host-enabled debug build self-registers the Explorer menu through `reg`.
    throw '-WithHost requires a stub reg.exe next to the candidate exe.'
}
New-Item -ItemType Directory -Path $evidence | Out-Null
$process = $null
$savedEnv = @{}
function Set-AppEnvironment([string]$Name, [string]$Value) {
    $savedEnv[$Name] = [Environment]::GetEnvironmentVariable($Name, 'Process')
    # A [string] parameter turns $null into '' and '' would still define the
    # variable (KAMIN_CEF_FORCE_SW='' forces software CEF): remove it instead.
    $effective = if ([string]::IsNullOrEmpty($Value)) { [NullString]::Value } else { $Value }
    [Environment]::SetEnvironmentVariable($Name, $effective, 'Process')
}
$device = if ($Warp) { 'warp' } elseif ($DeviceLoss) { 'd3d11on12' } else { $null }
try {
    foreach ($name in @('LOCALAPPDATA', 'APPDATA', 'TEMP', 'TMP', 'USERPROFILE')) {
        $profileDir = Join-Path $evidence $name
        New-Item -ItemType Directory -Path $profileDir | Out-Null
        Set-AppEnvironment $name $profileDir
    }
    Set-AppEnvironment 'HOME' (Join-Path $evidence 'USERPROFILE')
    Set-AppEnvironment 'KAMIN_PROBE_PORT' ([string]$Port)
    Set-AppEnvironment 'KAMIN_DEBUG_LOADERS' '1'
    Set-AppEnvironment 'KAMIN_DEBUG_NO_HOST' $(if ($WithHost) { '0' } else { '1' })
    Set-AppEnvironment 'KAMIN_DEBUG_NO_ACTIVATE' '1'
    Set-AppEnvironment 'KAMIN_DEBUG_REDUCED_ANIMATIONS' $(if ($ReducedAnimations) { '1' } else { '0' })
    Set-AppEnvironment 'KAMIN_DEBUG_SOFTWARE_POLICY' $(if ($Warp) { '1' } else { '0' })
    Set-AppEnvironment 'KAMIN_DEBUG_D3D_DEVICE' $device
    Set-AppEnvironment 'KAMIN_CEF_FORCE_SW' $(if ($SoftwareCef) { '1' } else { $null })
    Set-AppEnvironment 'KAMIN_DEV_REPO' $repo
    $manifest = [ordered]@{ candidate_sha = (git -C $repo rev-parse HEAD); source_exe_sha256 = (Get-FileHash -LiteralPath $candidate).Hash;
       port = $Port; d3d_device = $(if ($device) { $device } else { 'default' }); reduced_animations = [bool]$ReducedAnimations;
       software_cef = [bool]$SoftwareCef; with_host = [bool]$WithHost; no_activate = $true;
       started_utc = [DateTime]::UtcNow.ToString('o') }
    # This is the interactive acceptance GUI, not a background helper: Hidden
    # also hides GPUI's first window. KAMIN_DEBUG_NO_ACTIVATE shows it without
    # taking the foreground from the operator's current application.
    $process = Start-Process -FilePath $candidate -WorkingDirectory $repo -PassThru -WindowStyle Normal `
        -RedirectStandardOutput (Join-Path $evidence 'stdout.log') -RedirectStandardError (Join-Path $evidence 'stderr.log')
    $manifest['pid'] = $process.Id
    $manifest | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidence 'manifest.json')
    Write-Host "Acceptance PID=$($process.Id), probe=$Port, d3d_device=$($manifest['d3d_device'])."
} finally {
    # Restore the launcher's process environment as soon as the child inherits it.
    foreach ($name in $savedEnv.Keys) { [Environment]::SetEnvironmentVariable($name, $savedEnv[$name], 'Process') }
}
if ($Wait -and $process) { $process.WaitForExit() }
