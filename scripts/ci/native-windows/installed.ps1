# Install the verified release package in the disposable runner profile and start its shipped binary.
[CmdletBinding()]
param([Parameter(Mandatory)][string]$PackageDir)
$ErrorActionPreference = 'Stop'
if ($env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows') { throw 'Requires a disposable hosted Windows runner' }
$evidence = Join-Path $env:RUNNER_TEMP 'installed-client-report'
$capture = Join-Path $env:RUNNER_TEMP ('installed-client-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $capture,$evidence | Out-Null
$app = $null
$setup = $null
$runner = $null
$stage = 'provenance'
$checks = [ordered]@{ provenance = $false; previousVersion = $false; inAppUpdateAndRestart = $false
    versionFile = $false; displayVersion = $false; explorerRegistration = $false; installationLog = $false }
try {
    $packages = @(Get-ChildItem -Path $PackageDir -Filter 'KaminIDE_*_x64-setup.exe')
    $records = @(Get-ChildItem -Path $PackageDir -Filter 'KaminIDE_*_provenance.json')
    if ($packages.Count -ne 1 -or $records.Count -ne 1) { throw 'Expected one installer and one provenance record' }
    $package = $packages[0]
    $metadata = Get-Content -Raw $records[0].FullName | ConvertFrom-Json
    node scripts/release/provenance-cli.mjs verify --installer $package.FullName --provenance $records[0].FullName --release-sha $env:SMOKE_SHA
    if ($LASTEXITCODE) { throw 'Installer provenance failed' }
    $checks.provenance = $true
    $stage = 'empty-runner-profile'
    $registry = 'HKCU:\Software\KaminIDE-GPUI'
    if (Test-Path $registry) { throw 'Installation already present in runner account' }
    foreach ($name in @('LOCALAPPDATA','APPDATA','USERPROFILE','TEMP')) {
        $value = Join-Path $capture $name
        New-Item -ItemType Directory -Path $value | Out-Null
        [Environment]::SetEnvironmentVariable($name, $value, 'Process')
    }
    # Rust GetTempPath and PowerShell must resolve the same disposable log directory.
    $env:TMP = $env:TEMP
    $env:HOME = $env:USERPROFILE
    $stage = 'install-previous-release'
    $baselineDir = Join-Path $PackageDir 'baseline'
    $previous = (Get-Content (Join-Path $baselineDir 'version.txt') -Raw).Trim()
    $baselineExe = Join-Path $baselineDir "KaminIDE_${previous}_x64-setup.exe"
    $setup = Start-Process -FilePath $baselineExe -ArgumentList '/S' -PassThru
    if (!$setup.WaitForExit(120000) -or $setup.ExitCode -ne 0) { throw 'Silent installer failed or timed out' }
    # The setup may break out of the invoking job; its own final marker certifies completion.
    $installLog = Join-Path $env:TEMP 'kaminide-install.log'
    $deadline = [DateTime]::UtcNow.AddSeconds(100)
    while (!(Test-Path $installLog) -or !(Select-String -Quiet -SimpleMatch 'install done' -Path $installLog)) {
        if ([DateTime]::UtcNow -gt $deadline) { throw 'Installer did not finish in this profile' }
        Start-Sleep -Milliseconds 500
    }
    $stage = 'locate-installed-client'
    $installDir = (Get-ItemProperty -Path $registry -Name InstallDir).InstallDir
    $expected = Join-Path $env:LOCALAPPDATA 'Programs\KaminIDE-GPUI'
    if ([IO.Path]::GetFullPath($installDir) -ne $expected) { throw 'InstallDir points outside the fresh profile' }
    $exe = (Resolve-Path -LiteralPath (Join-Path $installDir 'kaminide-gpui.exe')).Path
    if (!(Test-Path (Join-Path $installDir 'runtime\kamin-host.mjs'))) { throw 'Packaged Extension Host is missing' }
    $env:KAMIN_PROBE_PORT = '19357'
    $env:KAMIN_PROBE_TOKEN = [guid]::NewGuid().ToString('N')
    $versionFile = Join-Path $installDir 'version.txt'
    if ((Get-Content $versionFile -Raw).Trim() -ne $previous) { throw 'Previous release did not install' }
    $checks.previousVersion = $true
    $stage = 'in-app-update'
    node (Join-Path $PSScriptRoot 'upgrade.mjs') $exe $package.FullName $previous $metadata.versions.app $evidence $capture
    if ($LASTEXITCODE) { throw 'In-app update or automatic restart failed. See upgrade.json.' }
    $app = Get-Process -Name 'kaminide-gpui' | Where-Object { $_.Path -eq $exe } | Select-Object -First 1
    if (!$app) { throw 'Restarted executable was not found in the installation directory' }
    $checks.inAppUpdateAndRestart = $true
    $stage = 'verify-upgraded-installation'
    if ((Get-Content $versionFile -Raw).Trim() -ne $metadata.versions.app) { throw 'Installed version file mismatch' }
    $checks.versionFile = $true
    $display = Get-ItemPropertyValue 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\KaminIDE-GPUI' -Name DisplayVersion
    if ($display -ne $metadata.versions.app) { throw 'Uninstall registry version mismatch' }
    $checks.displayVersion = $true
    foreach ($key in @('Directory\shell','Directory\Background\shell')) {
        $menu = Get-Item "HKCU:\Software\Classes\$key\OpenWithKaminIDE"
        $command = Get-Item (Join-Path $menu.PSPath 'command')
        if ($menu.GetValue('') -ne 'Open with KaminIDE' -or $command.GetValue('') -ne ('"' + $exe + '" "%V"')) {
            throw 'Open with KaminIDE registration mismatch'
        }
    }
    $checks.explorerRegistration = $true
    $logLines = Get-Content $installLog
    $versionMarker = [regex]::Escape("[" + $metadata.versions.app + "]")
    if (!($logLines -match ($versionMarker + ' install done')) -or ($logLines -match ($versionMarker + ' install FAILED'))) {
        throw 'Release installation log did not confirm success'
    }
    $checks.installationLog = $true
    [ordered]@{ candidate_sha = $env:SMOKE_SHA; source_exe_sha256 = (Get-FileHash $exe).Hash
        version = $metadata.versions.app; source = $env:INSTALLER_SOURCE; installer_sha256 = $metadata.installer.sha256
    } | ConvertTo-Json | Set-Content (Join-Path $capture 'manifest.json')
    $stage = 'exercise-installed-client'
    if ($env:INSTALLED_SCENARIO -ne 'native-client') {
        $runner = & (Join-Path $PSScriptRoot 'start-bridge.ps1') -DataDirectory (Join-Path $capture 'bridge')
    }
    node (Join-Path $PSScriptRoot 'probe-smoke.mjs') installed-client $capture $evidence
    if ($LASTEXITCODE) { throw 'Installed-client boundary failed. See report.json.' }
    $stage = 'complete'
} finally {
    [ordered]@{ candidate = $env:SMOKE_SHA; driver = $env:DRIVER_SHA; stage = $stage
        source = $env:INSTALLER_SOURCE; version = $metadata.versions.app; previous = $previous
        checks = $checks; explorerUi = "registration only"; versionText = "inspect version.png when available"
        installerSha256 = $metadata.installer.sha256; runner = $env:ImageVersion
        session = (Get-Process -Id $PID).SessionId; interactive = [Environment]::UserInteractive
        clientExit = if ($app -and $app.HasExited) { $app.ExitCode } else { $null }
    } | ConvertTo-Json | Set-Content (Join-Path $evidence 'pipeline.json')
    $owned = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($capture + '\', [StringComparison]::OrdinalIgnoreCase) })
    foreach ($child in @($app, $setup, $runner) + $owned) {
        if ($child -and !$child.HasExited) { taskkill /PID $child.Id /T /F | Out-Null }
    }
    # Chromium can release its memory-mapped metrics file after the process tree exits.
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    while (Test-Path -LiteralPath $capture) {
        try { Remove-Item -LiteralPath $capture -Recurse -Force -ErrorAction Stop }
        catch {
            if ([DateTime]::UtcNow -gt $deadline) { throw 'Disposable installer profile could not be removed' }
            Start-Sleep -Milliseconds 500
        }
    }
}

# The profile was removed. A taskkill race on an already exited child is harmless.
exit 0
