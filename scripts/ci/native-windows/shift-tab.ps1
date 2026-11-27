# Only the main HWND of the verified client may receive keyboard input.
[CmdletBinding()]
param([Parameter(Mandatory)][ValidateSet('Observe','Activate','Send')][string]$Action)
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class SmokeForeground {
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr hwnd);
    private delegate bool WindowCallback(IntPtr hwnd, IntPtr data);
    [DllImport("user32.dll")]
    private static extern bool EnumWindows(WindowCallback callback, IntPtr data);
    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll", EntryPoint = "GetWindowLongW")]
    private static extern int GetWindowLong(IntPtr hwnd, int index);
    [StructLayout(LayoutKind.Sequential)]
    private struct Rect { public int left, top, right, bottom; }
    [DllImport("user32.dll")]
    private static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);

    // Match the native probe: largest visible non-tool window owned by this PID.
    public static IntPtr MainWindow(uint targetPid) {
        IntPtr best = IntPtr.Zero;
        long largest = 0;
        EnumWindows(delegate(IntPtr hwnd, IntPtr unused) {
            uint pid;
            Rect rect;
            GetWindowThreadProcessId(hwnd, out pid);
            if (pid == targetPid && IsWindowVisible(hwnd) &&
                (GetWindowLong(hwnd, -20) & 0x80) == 0 && GetWindowRect(hwnd, out rect)) {
                long size = (long)(rect.right - rect.left) * (rect.bottom - rect.top);
                if (size > largest) { largest = size; best = hwnd; }
            }
            return true;
        }, IntPtr.Zero);
        return best;
    }
}
'@
$app = Get-Process -Id ([int]$env:SMOKE_APP_PID)
$app.Refresh()
$main = [SmokeForeground]::MainWindow([uint32]$app.Id)
$visible = $main -ne [IntPtr]::Zero
$before = $visible -and [SmokeForeground]::GetForegroundWindow() -eq $main
$activated = $null
if ($Action -eq 'Activate') {
    $keys = New-Object -ComObject WScript.Shell
    $activated = $keys.AppActivate([int]$app.Id)
} elseif ($Action -eq 'Send' -and $before) {
    $keys = New-Object -ComObject WScript.Shell
    $keys.SendKeys('+{TAB}')
}
$after = $visible -and [SmokeForeground]::GetForegroundWindow() -eq $main
@{ windowVisible = $visible; matchesDotNetMain = $visible -and $app.MainWindowHandle -eq $main; before = $before; appActivate = $activated; after = $after } | ConvertTo-Json -Compress
