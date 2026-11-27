# A registered WSL 2 distribution and a working Linux kernel are separate observations.
$distributions = @(Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Lxss' -ErrorAction SilentlyContinue |
    Get-ItemProperty | Where-Object { $_.DistributionName -and $_.Version -eq 2 })
$wsl = Get-Command wsl.exe -ErrorAction SilentlyContinue
$feature = Get-CimInstance Win32_OptionalFeature -Filter "Name='Microsoft-Windows-Subsystem-Linux'" -ErrorAction SilentlyContinue
$platform = Get-CimInstance Win32_OptionalFeature -Filter "Name='VirtualMachinePlatform'" -ErrorAction SilentlyContinue
$started = $false
if ($wsl -and $distributions.Count -gt 0 -and $env:RUNNER_ENVIRONMENT -eq 'github-hosted') {
    $name = $distributions[0].DistributionName
    $job = Start-Job -ArgumentList $name -ScriptBlock {
        param($distribution)
        $kernel = & wsl.exe --distribution $distribution --exec uname -s 2>$null
        return $LASTEXITCODE -eq 0 -and ($kernel -join '').Trim() -eq 'Linux'
    }
    try {
        if (Wait-Job $job -Timeout 12) { $started = (Receive-Job $job) -eq $true }
    } finally {
        Remove-Job $job -Force -ErrorAction SilentlyContinue
    }
}
return [ordered]@{
    executable = [bool]$wsl; featureEnabled = $feature.InstallState -eq 1
    virtualMachinePlatform = $platform.InstallState -eq 1; registeredWsl2 = $distributions.Count; kernelStarted = $started
}
