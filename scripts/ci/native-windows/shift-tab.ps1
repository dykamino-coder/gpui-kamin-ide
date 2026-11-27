# Dispatch through the foreground application on the disposable hosted desktop.
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class SmokeForeground {
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
}
'@
$main = (Get-Process -Id $env:SMOKE_APP_PID).MainWindowHandle
$before = $main -ne [IntPtr]::Zero -and [SmokeForeground]::GetForegroundWindow() -eq $main
if ($before) {
    $keys = New-Object -ComObject WScript.Shell
    $keys.SendKeys('+{TAB}')
}
@{ before = $before; after = ([SmokeForeground]::GetForegroundWindow() -eq $main) } | ConvertTo-Json -Compress
