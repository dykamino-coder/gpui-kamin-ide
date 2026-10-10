# Launch a disposable DEBUG candidate; restore the two app-scoped D3D overrides.
# Requires Windows Graphics Tools only for -Warp / -DeviceLoss.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$EvidenceDir,
    [int]$Port = 9357,
    [switch]$Warp,
    [switch]$DeviceLoss,
    [switch]$ReducedAnimations,
    [switch]$SoftwareCef,
    [switch]$WithHost
)
$ErrorActionPreference = 'Stop'
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
New-Item -ItemType Directory -Path $evidence | Out-Null
$appName = "kaminide-native-acceptance-$PID.exe"
$appExe = Join-Path (Split-Path -Parent $candidate) $appName
if (Test-Path -LiteralPath $appExe) { throw 'Acceptance executable already exists.' }
$d3d = $null
$original = @{}
$added = $false
$process = $null
$savedEnv = @{}
function Set-AppEnvironment([string]$Name, [string]$Value) {
    $savedEnv[$Name] = [Environment]::GetEnvironmentVariable($Name, 'Process')
    [Environment]::SetEnvironmentVariable($Name, $Value, 'Process')
}
function Invoke-D3D([string[]]$Arguments) {
    $output = & $d3d @Arguments 2>&1
    $code = $LASTEXITCODE
    $output | Out-File -LiteralPath (Join-Path $evidence 'd3dconfig.log') -Append -Encoding utf8
    if ($code -ne 0) { throw 'D3DConfig failed; see private d3dconfig.log.' }
    return $output
}
try {
    Copy-Item -LiteralPath $candidate -Destination $appExe
    if ($Warp -or $DeviceLoss) {
        $d3d = (Get-Command d3dconfig.exe -ErrorAction Stop).Source
        $apps = (Invoke-D3D -Arguments @('apps')) -join "`n"
        # Settings apply to ALL listed apps. Refuse an existing list instead of
        # forcing another application or replacing the user's configuration.
        if ($apps -notmatch '<no apps>') { throw 'D3DConfig has existing apps; use a disposable Windows account with an empty app list.' }
        $settings = (Invoke-D3D -Arguments @('device')) -join "`n"
        foreach ($setting in @('force-warp', 'force-d3d11on12')) {
            $settingMatch = [regex]::Match($settings, "(?m)^$setting=(true|false)\s*$")
            if (!$settingMatch.Success) { throw 'Cannot preserve D3DConfig settings.' }
            $original[$setting] = $settingMatch.Groups[1].Value
        }
        Invoke-D3D -Arguments @('apps', '--add', $appExe) | Out-Null
        $added = $true
        if ($Warp) { Invoke-D3D -Arguments @('device', 'force-warp=true') | Out-Null }
        if ($DeviceLoss) { Invoke-D3D -Arguments @('device', 'force-d3d11on12=true') | Out-Null }
    }
    foreach ($name in @('LOCALAPPDATA', 'APPDATA', 'TEMP', 'TMP')) {
        $profileDir = Join-Path $evidence $name
        New-Item -ItemType Directory -Path $profileDir | Out-Null
        Set-AppEnvironment $name $profileDir
    }
    Set-AppEnvironment 'KAMIN_PROBE_PORT' ([string]$Port)
    Set-AppEnvironment 'KAMIN_DEBUG_LOADERS' '1'
    Set-AppEnvironment 'KAMIN_DEBUG_NO_HOST' $(if ($WithHost) { '0' } else { '1' })
    Set-AppEnvironment 'KAMIN_DEBUG_REDUCED_ANIMATIONS' $(if ($ReducedAnimations) { '1' } else { '0' })
    Set-AppEnvironment 'KAMIN_DEBUG_SOFTWARE_POLICY' $(if ($Warp) { '1' } else { '0' })
    Set-AppEnvironment 'KAMIN_CEF_FORCE_SW' $(if ($SoftwareCef) { '1' } else { $null })
    Set-AppEnvironment 'KAMIN_DEV_REPO' $repo
    @{ candidate_sha = (git -C $repo rev-parse HEAD); source_exe_sha256 = (Get-FileHash -LiteralPath $candidate).Hash;
       port = $Port; warp_requested = [bool]$Warp; device_loss_supported_requested = [bool]$DeviceLoss;
       reduced_animations = [bool]$ReducedAnimations; software_cef = [bool]$SoftwareCef;
       started_utc = [DateTime]::UtcNow.ToString('o') } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidence 'manifest.json')
    # This is the interactive acceptance GUI, not a background helper: Hidden
    # also hides GPUI's first window and makes visible-loader acceptance fail.
    $process = Start-Process -FilePath $appExe -WorkingDirectory $repo -PassThru -WindowStyle Normal `
        -RedirectStandardOutput (Join-Path $evidence 'stdout.log') -RedirectStandardError (Join-Path $evidence 'stderr.log')
    # Restore the launcher's process environment as soon as the child inherits it.
    foreach ($name in $savedEnv.Keys) { [Environment]::SetEnvironmentVariable($name, $savedEnv[$name], 'Process') }
    Write-Host "Acceptance PID=$($process.Id), probe=$Port. Close the application to restore D3DConfig."
    $process.WaitForExit()
} finally {
    foreach ($name in $savedEnv.Keys) { [Environment]::SetEnvironmentVariable($name, $savedEnv[$name], 'Process') }
    if ($process -and !$process.HasExited) { $process.WaitForExit() }
    if ($added) {
        # Even if restoring one setting fails, still remove the exact test app.
        try {
            foreach ($setting in $original.Keys) { Invoke-D3D -Arguments @('device', "$setting=$($original[$setting])") | Out-Null }
        } finally { Invoke-D3D -Arguments @('apps', '--remove', $appExe) | Out-Null }
    }
    if (Test-Path -LiteralPath $appExe) { Remove-Item -LiteralPath $appExe }
}
