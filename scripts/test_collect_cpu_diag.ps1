# Synthetic collector regression tests; no running KaminIDE or private logs used.
$ErrorActionPreference = 'Stop'
$source = [IO.File]::ReadAllText((Join-Path $PSScriptRoot 'collect_cpu_diag.ps1'), [Text.Encoding]::UTF8)
$tokens = $null; $errors = $null
$ast = [Management.Automation.Language.Parser]::ParseInput($source, [ref]$tokens, [ref]$errors)
if ($errors.Count) { throw 'Collector parser failed' }
foreach ($definition in $ast.FindAll({ param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] }, $false)) {
    . ([ScriptBlock]::Create($definition.Extent.Text))
}
function Assert($Condition, [string]$Message) { if (-not $Condition) { throw $Message } }

$fixtureRoot = Join-Path ([IO.Path]::GetTempPath()) ('kamin-collector-test-' + [Guid]::NewGuid().ToString('N'))
$Out = Join-Path $fixtureRoot 'capture'
New-Item -ItemType Directory -Path $Out -Force | Out-Null
$envNames = @('KAMIN_REVIEW_SECRET', 'KAMIN_API_KEY', 'KAMIN_PROBE_TOKEN') + @(Get-DiagnosticEnvNames)
$oldEnv = @{}
foreach ($name in $envNames) { $oldEnv[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
$sentinel = 'SYNTHETIC_CREDENTIAL_SENTINEL'
try {
    foreach ($name in $envNames) { [Environment]::SetEnvironmentVariable($name, $sentinel, 'Process') }
    $env:KAMIN_REDUCE_MOTION = '1'
    Assert ((Format-DiagnosticEnvValue 'KAMIN_API_KEY' '1') -eq '[REDACTED]') 'Sensitive name was not redacted'
    Assert ((Format-DiagnosticEnvValue 'KAMIN_FORCE_SW_RENDER' $sentinel) -eq '[REDACTED]') 'Invalid flag value was exported'
    $overrides = ConvertTo-DiagnosticOverrides 'KAMIN_REDUCE_MOTION=0;KAMIN_FORCE_SW_RENDER=1'
    Assert ($overrides.Count -eq 2 -and $overrides.KAMIN_REDUCE_MOTION -eq '0') 'Valid overrides rejected'
    foreach ($spec in @("KAMIN_API_KEY=$sentinel", "KAMIN_REDUCE_MOTION=$sentinel", 'PATH=1', 'broken')) {
        $rejected = $false
        $restartOutput = & {
            try { Restart-Kamin $spec } catch { $script:restartError = $_.Exception.Message; $script:restartRejected = $true }
        } *>&1 | Out-String
        $rejected = $script:restartRejected
        Assert $rejected 'Unsafe restart was accepted'
        Assert (-not ($restartOutput + $script:restartError).Contains($sentinel)) 'Restart output leaked input'
        $script:restartRejected = $false
    }
    Assert ((Get-RendererObservation @() $false) -like 'unknown:*') 'Failed module read claimed renderer'
    Assert ((Get-RendererObservation @() $true) -like 'unknown:*') 'Empty module read claimed renderer'
    Assert ((Get-RendererObservation @([pscustomobject]@{ModuleName='dxgi.dll'}) $true) -match 'не доказана') 'Absent WARP claimed hardware'
    Assert ((Get-RendererObservation @([pscustomobject]@{ModuleName='d3d10warp.dll'}) $true) -match 'отдельно не установлен') 'Loaded WARP claimed active renderer'

    $list = New-Object 'Collections.Generic.List[string]'
    $list.Add('one'); $list.Add('two'); Save 'newlines.txt' $list
    Assert ([IO.File]::ReadAllText((Join-Path $Out 'newlines.txt')) -eq "one`r`ntwo") 'Save lost line boundaries'
    $log = Join-Path $fixtureRoot 'synthetic.log'
    [IO.File]::WriteAllText($log, 'old-new')
    Assert ((Read-NewText $log 4) -eq 'new') 'Offset read failed'
    [IO.File]::WriteAllText($log, 'x')
    Assert ((Read-NewText $log 4) -eq 'x') 'Truncated read failed'
    $diag = '[cef] за секунду: кадров 0, кадров окна 30, отрисовок 30, заказов 30; окно сложило 30 кадров: сцена 200 мс, презент 4 мс'
    $stats = @(Get-DiagStats $diag)
    Assert ($stats.Count -eq 1 -and (Avg $stats WinDraws 2) -eq 15) 'Diag stats/idle denominator failed'

    # Exercise the actual collection/summary/archive code against this test
    # process. OS/probe calls and log intake use synthetic data only.
    function Invoke-Probe { return $null }
    function Get-CimInstance { return [pscustomobject]@{TotalPhysicalMemory=0; LoadPercentage=0} }
    function qwinsta { 'synthetic' }
    function powercfg { 'synthetic' }
    function Get-MainProc { [pscustomobject]@{ProcessId=$PID; ExecutablePath='synthetic.exe'} }
    function Get-KaminProcs { [pscustomobject]@{ProcessId=$PID; Name='kaminide-gpui.exe'; WorkingSetSize=0; CommandLine='synthetic'} }
    function Get-GpuSample { return $null }
    function Read-NewText { return $diag }
    function Invoke-PageProbe { [pscustomobject]@{Log=$null; Sent=@(); Results=@()} }
    $CacheDir = Join-Path $fixtureRoot 'absent-cache'
    $DataDir = Join-Path $fixtureRoot 'absent-data'
    $DefaultExe = Join-Path $fixtureRoot 'absent.exe'
    $Label = 'synthetic'; $Nonce = 'synthetic'; $SampleSeconds = 1
    $RestartWith = ''; $NoScreenshot = $true; $NoZip = $false
    $Summary = New-Object 'Collections.Generic.List[string]'
    $programStart = $source.IndexOf('Write-Host "Папка пакета:')
    Assert ($programStart -gt 0) 'Collection entry point missing'
    $output = & ([ScriptBlock]::Create($source.Substring($programStart))) *>&1 | Out-String
    Assert (-not $output.Contains($sentinel)) 'stdout leaked credential'
    $summary = [IO.File]::ReadAllText((Join-Path $Out 'summary.txt'))
    Assert ($summary -match 'причина не установлена') 'Summary claimed frame cause'
    Assert ($summary -notmatch 'кадры заказывает НАТИВНАЯ') 'Summary claimed native animation'
    $envReport = [IO.File]::ReadAllText((Join-Path $Out 'env.txt'))
    Assert ($envReport.Contains('[Process] KAMIN_REDUCE_MOTION=1')) 'Allow-listed flag missing'
    Assert ($envReport.Contains('[REDACTED]')) 'Invalid allow-listed value not redacted'
    Assert (-not $envReport.Contains('KAMIN_REVIEW_SECRET')) 'Unknown variable exported'
    Assert (Test-Path "$Out.zip") 'Archive missing'
    $expanded = Join-Path $fixtureRoot 'expanded'
    Expand-Archive -Path "$Out.zip" -DestinationPath $expanded
    foreach ($file in (Get-ChildItem $expanded -File -Recurse)) {
        Assert (-not ([IO.File]::ReadAllText($file.FullName)).Contains($sentinel)) 'Archive leaked credential'
    }
    Write-Output "PowerShell $($PSVersionTable.PSVersion): parser, privacy, restart, diagnostic output, synthetic collection/archive PASS"
} finally {
    foreach ($name in $envNames) { [Environment]::SetEnvironmentVariable($name, $oldEnv[$name], 'Process') }
    # This verified, unique fixture lives under TEMP and contains only synthetic data.
    $resolvedFixture = [IO.Path]::GetFullPath($fixtureRoot)
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    if (-not $resolvedFixture.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe fixture cleanup path' }
    Remove-Item -LiteralPath $resolvedFixture -Recurse -Force
}
