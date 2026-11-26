# Remove only the scenario's new profile beneath the hosted runner's temporary directory.
[CmdletBinding()]
param([Parameter(Mandatory)][string]$Directory, [object[]]$Processes)
$ErrorActionPreference = 'Stop'
$temp = [IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\') + '\'
if (![IO.Path]::GetFullPath($Directory).StartsWith($temp, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Expected a disposable runner profile'
}
foreach ($child in $Processes) {
    if ($child -and !$child.HasExited) { taskkill /PID $child.Id /T /F | Out-Null }
}
# Chromium can release its mapped metrics file after its process tree terminates.
$deadline = [DateTime]::UtcNow.AddSeconds(20)
while (Test-Path -LiteralPath $Directory) {
    try { Remove-Item -LiteralPath $Directory -Recurse -Force -ErrorAction Stop }
    catch {
        if ([DateTime]::UtcNow -gt $deadline) { throw 'Disposable profile could not be removed' }
        Start-Sleep -Milliseconds 500
    }
}
