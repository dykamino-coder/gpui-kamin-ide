<#
.SYNOPSIS
  Пакет диагностики «KaminIDE грузит CPU/GPU и всё висит» (INC-2026-0001).

.DESCRIPTION
  Снимает с ЖИВОГО приложения, кто и почему постоянно рисует кадры:

    * окружение: адаптеры, RDP, системный «эффекты анимации» (из него Chromium
      берёт prefers-reduced-motion), KAMIN_* переменные, версия;
    * CPU всех процессов и CPU потоков главного процесса за окно простоя, с
      привязкой потока к модулю (d3d10warp.dll = программная растеризация);
    * загрузку 3D-движка GPU по процессам (на машинах с видеокартой);
    * строки diag.log, записанные именно за окно простоя: кадры Chromium против
      кадров окна gpui. Кадры окна при нуле кадров Chromium = анимация самого
      нативного интерфейса (лоадер, спиннер, каретка, пульс статуса);
    * зонд внутри каждой страницы CEF: какие CSS/JS-анимации живут, видимы ли
      их элементы, prefers-reduced-motion, сработало ли наше глушение;
    * скриншот окна и хвосты логов.

  Во время окна замера мышь и клавиатуру НЕ трогать.

  Сравнительные прогоны (каждый — отдельный архив):
    .\collect_cpu_diag.ps1 -Label base
    .\collect_cpu_diag.ps1 -Label rm   -RestartWith KAMIN_REDUCE_MOTION=1
    .\collect_cpu_diag.ps1 -Label sw   -RestartWith KAMIN_FORCE_SW_RENDER=1   # на машине С видеокартой

  Запуск, если PowerShell запрещает скрипты:
    powershell -ExecutionPolicy Bypass -File .\collect_cpu_diag.ps1 -Label base

.PARAMETER Label
  Метка прогона: попадает в имя папки и архива.
.PARAMETER SampleSeconds
  Длина окна простоя, в котором меряется CPU/GPU и вырезается diag.log.
.PARAMETER RestartWith
  Перезапустить приложение с переменными окружения "ИМЯ=значение[;ИМЯ=значение]"
  и мерить уже новый процесс. Сессии Claude живут на сервере, но открытые
  несохранённые правки в редакторе нужно сохранить заранее.
.PARAMETER WarmupSeconds
  Сколько ждать после перезапуска, прежде чем мерить (успеть открыть тот же экран).
.PARAMETER NoScreenshot
  Не снимать скриншот окна (на нём видно содержимое чата и файлов).
#>
[CmdletBinding()]
param(
    [string]$Label = 'base',
    [int]$SampleSeconds = 30,
    [string]$RestartWith = '',
    [int]$WarmupSeconds = 60,
    [string]$OutRoot = (Join-Path ([Environment]::GetFolderPath('Desktop')) 'kamin-cpu-diag'),
    [int]$ProbePort = 9333,
    [switch]$NoScreenshot,
    [switch]$NoZip
)

$ErrorActionPreference = 'Continue'
$ProgressPreference = 'SilentlyContinue'
try { [Console]::OutputEncoding = [Text.Encoding]::UTF8 } catch {}

$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$safeLabel = ($Label -replace '[^\w\-]', '_')
$Out = Join-Path $OutRoot "$stamp-$env:COMPUTERNAME-$safeLabel"
New-Item -ItemType Directory -Force -Path $Out | Out-Null

$KaminBase = Join-Path $env:LOCALAPPDATA 'kaminide-gpui-dev'
$CacheDir = Join-Path $KaminBase 'cache'
$DataDir = Join-Path $KaminBase 'data'
$DefaultExe = Join-Path $env:LOCALAPPDATA 'Programs\KaminIDE-GPUI\kaminide-gpui.exe'
$Nonce = [Guid]::NewGuid().ToString('N').Substring(0, 8)
$Summary = New-Object System.Collections.Generic.List[string]

function Say([string]$Text) { Write-Host $Text; $Summary.Add($Text) | Out-Null }
function Save([string]$Name, $Content) {
    $path = Join-Path $Out $Name
    # List<string> не [array]: без явной склейки строки слипались через пробел.
    if ($Content -isnot [string] -and $Content -is [Collections.IEnumerable]) { $Content = @($Content) -join "`r`n" }
    [IO.File]::WriteAllText($path, [string]$Content, (New-Object Text.UTF8Encoding($false)))
}
function Section([string]$Title) { "`r`n=== $Title ===`r`n" }

Add-Type -Namespace KDiag -Name Native -MemberDefinition @'
[DllImport("user32.dll")] public static extern int GetSystemMetrics(int n);
[DllImport("user32.dll")] public static extern bool SystemParametersInfo(uint action, uint param, ref bool value, uint ini);
[DllImport("kernel32.dll")] public static extern IntPtr OpenThread(uint access, bool inherit, uint id);
[DllImport("kernel32.dll")] public static extern int GetThreadDescription(IntPtr h, out IntPtr desc);
[DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr h);
[DllImport("kernel32.dll")] public static extern IntPtr LocalFree(IntPtr p);
'@ -ErrorAction SilentlyContinue

# --- probe-канал приложения (127.0.0.1:9333, построчный JSON) ---------------

function Invoke-Probe([hashtable]$Req, [int]$TimeoutMs = 5000) {
    if ($env:KAMIN_PROBE_TOKEN) { $Req['token'] = $env:KAMIN_PROBE_TOKEN }
    $client = New-Object System.Net.Sockets.TcpClient
    try {
        $iar = $client.BeginConnect('127.0.0.1', $ProbePort, $null, $null)
        if (-not $iar.AsyncWaitHandle.WaitOne($TimeoutMs)) { return $null }
        $client.EndConnect($iar)
        $client.ReceiveTimeout = $TimeoutMs
        $stream = $client.GetStream()
        $bytes = [Text.Encoding]::UTF8.GetBytes(($Req | ConvertTo-Json -Compress -Depth 6) + "`n")
        $stream.Write($bytes, 0, $bytes.Length)
        $ms = New-Object IO.MemoryStream
        $buf = New-Object byte[] 65536
        while ($true) {
            $n = $stream.Read($buf, 0, $buf.Length)
            if ($n -le 0) { break }
            $ms.Write($buf, 0, $n)
            if ([Array]::IndexOf($buf, [byte]10, 0, $n) -ge 0) { break }
        }
        return [Text.Encoding]::UTF8.GetString($ms.ToArray()).Trim()
    } catch {
        return $null
    } finally {
        $client.Close()
    }
}

# --- процессы приложения ----------------------------------------------------

function Get-KaminProcs {
    Get-CimInstance Win32_Process -Filter "Name='kaminide-gpui.exe' OR Name='kaminhost.exe' OR Name='node.exe'" |
        Where-Object { $_.Name -ne 'node.exe' -or ($_.CommandLine -match 'kamin|claude-bridge') }
}

function Get-MainProc {
    Get-CimInstance Win32_Process -Filter "Name='kaminide-gpui.exe'" |
        Where-Object { $_.CommandLine -notmatch '--type=' } | Select-Object -First 1
}

function Get-ProcRole($cim) {
    if ($cim.Name -eq 'kaminide-gpui.exe') {
        if ($cim.CommandLine -match '--type=([\w-]+)') { return "cef:$($Matches[1])" }
        return 'MAIN'
    }
    return $cim.Name
}

# --- перезапуск с переменными окружения -------------------------------------

function Restart-Kamin([string]$Spec) {
    $main = Get-MainProc
    $exe = if ($main -and $main.ExecutablePath) { $main.ExecutablePath } else { $DefaultExe }
    if (-not (Test-Path $exe)) { Say "!! Не найден exe приложения: $exe"; return $false }

    Write-Host ''
    Write-Host "Приложение будет ПЕРЕЗАПУЩЕНО с: $Spec" -ForegroundColor Yellow
    Write-Host 'Сохраните несохранённые файлы в редакторе. Enter — продолжить, Ctrl+C — отмена.'
    [void](Read-Host)

    if ($main) {
        Invoke-Probe @{ cmd = 'flushLayout' } | Out-Null
        $p = Get-Process -Id $main.ProcessId -ErrorAction SilentlyContinue
        if ($p) {
            [void]$p.CloseMainWindow()
            if (-not $p.WaitForExit(20000)) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }
        }
        # Дочерние CEF-процессы и порт probe освобождаются не мгновенно; новый
        # экземпляр, увидев живой порт, отдал бы фокус старому и вышел.
        $deadline = (Get-Date).AddSeconds(20)
        while ((Get-Date) -lt $deadline -and (Get-Process -Name kaminide-gpui -ErrorAction SilentlyContinue)) {
            Start-Sleep -Milliseconds 500
        }
    }

    foreach ($pair in ($Spec -split ';')) {
        $kv = $pair.Split('=', 2)
        if ($kv.Count -eq 2 -and $kv[0].Trim()) {
            Set-Item -Path ("Env:" + $kv[0].Trim()) -Value $kv[1].Trim()
        }
    }
    Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) | Out-Null

    $deadline = (Get-Date).AddSeconds(120)
    while ((Get-Date) -lt $deadline) {
        if (Invoke-Probe @{ cmd = 'ping' } 2000) { break }
        Start-Sleep -Seconds 1
    }
    Write-Host ''
    Write-Host "Приложение запущено. За $WarmupSeconds с откройте тот же экран, что и в базовом прогоне," -ForegroundColor Yellow
    Write-Host 'потом отпустите мышь — замер начнётся сам.' -ForegroundColor Yellow
    for ($i = $WarmupSeconds; $i -gt 0; $i -= 5) { Write-Host "  ... $i с"; Start-Sleep -Seconds ([Math]::Min(5, $i)) }
    return $true
}

# --- окружение --------------------------------------------------------------

function Get-EnvReport {
    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add("Собрано: $(Get-Date -Format o)  метка: $Label  nonce: $Nonce")
    $verFile = Join-Path (Split-Path $DefaultExe) 'version.txt'
    if (Test-Path $verFile) { $lines.Add("version.txt: $((Get-Content $verFile -Raw).Trim())") }
    $ping = Invoke-Probe @{ cmd = 'ping' }
    $lines.Add("probe ping: $ping")

    $lines.Add((Section 'Сессия'))
    $lines.Add("SESSIONNAME = $env:SESSIONNAME")
    try { $lines.Add("SM_REMOTESESSION = $([KDiag.Native]::GetSystemMetrics(0x1000))  (не 0 = RDP: включается нативный reduce_motion)") } catch {}
    try {
        $anim = $false
        [void][KDiag.Native]::SystemParametersInfo(0x1042, 0, [ref]$anim, 0)
        $lines.Add("SPI_GETCLIENTAREAANIMATION = $anim  (False = Chromium считает prefers-reduced-motion: reduce)")
    } catch {}
    try { $lines.Add(((& qwinsta 2>$null) -join "`r`n")) } catch {}

    $lines.Add((Section 'Переменные KAMIN_* (этого окна PowerShell и пользователя)'))
    Get-ChildItem Env: | Where-Object { $_.Name -like 'KAMIN_*' -and $_.Name -ne 'KAMIN_PROBE_TOKEN' } |
        ForEach-Object { $lines.Add("$($_.Name)=$($_.Value)") }
    foreach ($scope in 'User', 'Machine') {
        foreach ($n in 'KAMIN_REDUCE_MOTION', 'KAMIN_FORCE_SW_RENDER', 'KAMIN_CEF_FORCE_SW') {
            $v = [Environment]::GetEnvironmentVariable($n, $scope)
            if ($v) { $lines.Add("[$scope] $n=$v") }
        }
    }

    $lines.Add((Section 'ОС / CPU / RAM'))
    $os = Get-CimInstance Win32_OperatingSystem
    $cs = Get-CimInstance Win32_ComputerSystem
    $lines.Add("$($os.Caption) $($os.Version) build $($os.BuildNumber)")
    $lines.Add("$($cs.Manufacturer) / $($cs.Model); RAM $([Math]::Round($cs.TotalPhysicalMemory / 1GB, 1)) GB")
    Get-CimInstance Win32_Processor | ForEach-Object {
        $lines.Add("$($_.Name): ядер $($_.NumberOfCores), потоков $($_.NumberOfLogicalProcessors), $($_.MaxClockSpeed) MHz")
    }
    try { $lines.Add("Схема питания: $((& powercfg /getactivescheme) -join ' ')") } catch {}

    $lines.Add((Section 'Видеоадаптеры (Win32_VideoController)'))
    Get-CimInstance Win32_VideoController | ForEach-Object {
        $lines.Add("$($_.Name) | драйвер $($_.DriverVersion) | $($_.CurrentHorizontalResolution)x$($_.CurrentVerticalResolution) @ $($_.CurrentRefreshRate) Гц | PNP $($_.PNPDeviceID)")
    }
    $lines.Add((Section 'Мониторы'))
    Add-Type -AssemblyName System.Windows.Forms -ErrorAction SilentlyContinue
    try { [System.Windows.Forms.Screen]::AllScreens | ForEach-Object { $lines.Add("$($_.DeviceName) $($_.Bounds) primary=$($_.Primary)") } } catch {}
    return $lines
}

# --- замер окна простоя ------------------------------------------------------

function Get-CpuSnapshot {
    $h = @{}
    foreach ($p in [Diagnostics.Process]::GetProcesses()) {
        try { $h[$p.Id] = @{ Name = $p.ProcessName; Ms = $p.TotalProcessorTime.TotalMilliseconds } } catch {}
    }
    return $h
}

function Get-ThreadSnapshot([int]$ProcId) {
    $h = @{}
    $p = Get-Process -Id $ProcId -ErrorAction SilentlyContinue
    if (-not $p) { return $h }
    foreach ($t in $p.Threads) {
        try { $h[$t.Id] = @{ Ms = $t.TotalProcessorTime.TotalMilliseconds; Start = $t.StartAddress.ToInt64() } } catch {}
    }
    return $h
}

function Get-ThreadName([int]$Tid) {
    try {
        $h = [KDiag.Native]::OpenThread(0x0800, $false, [uint32]$Tid)
        if ($h -eq [IntPtr]::Zero) { return '' }
        try {
            $ptr = [IntPtr]::Zero
            if ([KDiag.Native]::GetThreadDescription($h, [ref]$ptr) -ge 0 -and $ptr -ne [IntPtr]::Zero) {
                $s = [Runtime.InteropServices.Marshal]::PtrToStringUni($ptr)
                [void][KDiag.Native]::LocalFree($ptr)
                return $s
            }
        } finally { [void][KDiag.Native]::CloseHandle($h) }
    } catch {}
    return ''
}

function Get-GpuSample {
    # Счётчики GPU Engine: имена экземпляров вида pid_1234_..._engtype_3D.
    # WMI-класс не локализуется, в отличие от Get-Counter.
    try {
        Get-CimInstance Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine -ErrorAction Stop |
            Where-Object { $_.Name -match 'engtype_3D' -and $_.UtilizationPercentage -gt 0 } |
            ForEach-Object {
                if ($_.Name -match 'pid_(\d+)_') { [pscustomobject]@{ Pid = [int]$Matches[1]; Util = [int]$_.UtilizationPercentage } }
            }
    } catch { $null }
}

function Read-NewText([string]$Path, [long]$FromOffset) {
    if (-not (Test-Path $Path)) { return '' }
    $fs = [IO.File]::Open($Path, 'Open', 'Read', 'ReadWrite')
    try {
        if ($fs.Length -lt $FromOffset) { $FromOffset = 0 }  # файл перезапущен ротацией
        [void]$fs.Seek($FromOffset, 'Begin')
        $sr = New-Object IO.StreamReader($fs, [Text.Encoding]::UTF8)
        return $sr.ReadToEnd()
    } finally { $fs.Close() }
}

function Get-FileLength([string]$Path) { if (Test-Path $Path) { (Get-Item $Path).Length } else { 0 } }

function Copy-Tail([string]$Src, [string]$DstName, [int]$MaxBytes = 6MB) {
    if (-not (Test-Path $Src)) { return }
    $len = (Get-Item $Src).Length
    Save $DstName (Read-NewText $Src ([Math]::Max(0, $len - $MaxBytes)))
}

# --- зонд внутри страниц CEF -------------------------------------------------

# Пишет итог в консоль страницы маркером [kdiag]; CEF кладёт консоль в
# chrome_debug.log, ответ probe `wvjs` результата не возвращает.
$PageProbeJs = @'
(function(){var ID='__ID__',N='__NONCE__';function out(o){o.id=ID;o.nonce=N;try{console.log('[kdiag] '+JSON.stringify(o))}catch(e){}}
try{var t0=performance.now(),raf=0,mut=0,mo=null;
try{mo=new MutationObserver(function(l){mut+=l.length});mo.observe(document,{subtree:true,childList:true,attributes:true,characterData:true})}catch(e){}
function tick(){raf++;if(performance.now()-t0<2000)requestAnimationFrame(tick)}requestAnimationFrame(tick);
function desc(el){if(!el||!el.tagName)return null;var c=el.className;if(c&&typeof c!=='string')c=c.baseVal||'';return (el.tagName.toLowerCase()+(el.id?'#'+el.id:'')+(c?'.'+String(c).trim().split(/\s+/).slice(0,3).join('.'):'')).slice(0,120)}
function path(el){var p=[],n=el&&el.parentElement,i=0;while(n&&i<4){p.push(desc(n));n=n.parentElement;i++}return p.join(' < ')}
setTimeout(function(){if(mo)mo.disconnect();var list=[];try{list=document.getAnimations()}catch(e){}
var anims=[];for(var i=0;i<list.length&&i<60;i++){var a=list[i],eff=a.effect,el=eff&&eff.target,r=null,cs=null,ct=null,vis=null;
try{ct=eff.getComputedTiming()}catch(e){}try{r=el.getBoundingClientRect()}catch(e){}try{cs=getComputedStyle(el)}catch(e){}
try{vis=el.checkVisibility?el.checkVisibility({checkOpacity:true,checkVisibilityCSS:true}):null}catch(e){}
anims.push({name:a.animationName||a.transitionProperty||(a.constructor&&a.constructor.name),state:a.playState,iter:ct?String(ct.iterations):null,dur:ct?ct.duration:null,el:desc(el),parents:path(el),
rect:r?[Math.round(r.x),Math.round(r.y),Math.round(r.width),Math.round(r.height)]:null,
inViewport:r?(r.width>0&&r.height>0&&r.right>0&&r.bottom>0&&r.left<innerWidth&&r.top<innerHeight):null,visible:vis,opacity:cs?cs.opacity:null,iterCss:cs?cs.animationIterationCount:null})}
out({vis:document.visibilityState,hidden:document.hidden,focus:document.hasFocus(),reducedMotion:matchMedia('(prefers-reduced-motion: reduce)').matches,
injectedMute:!!document.getElementById('__kaminReducedMotion'),w:innerWidth,h:innerHeight,dpr:devicePixelRatio,rafPerSec:Math.round(raf/2),mutPerSec:Math.round(mut/2),
nAnim:list.length,anims:anims,url:String(location.href).slice(0,80)})},2100)}catch(e){out({err:String(e)})}})();
'@

function Find-CefConsoleLog {
    $candidates = @(
        (Join-Path $CacheDir 'cef\chrome_debug.log'),
        (Join-Path $CacheDir 'cef\debug.log'),
        (Join-Path $CacheDir 'chrome_debug.log'),
        (Join-Path (Split-Path $DefaultExe) 'debug.log')
    )
    $main = Get-MainProc
    if ($main -and $main.ExecutablePath) { $candidates += (Join-Path (Split-Path $main.ExecutablePath) 'debug.log') }
    $found = $candidates | Where-Object { Test-Path $_ } | Sort-Object { (Get-Item $_).LastWriteTime } -Descending
    return ($found | Select-Object -First 1)
}

function Invoke-PageProbe {
    $ids = New-Object System.Collections.Generic.List[string]
    foreach ($id in 'claudeBridgeChat', 'claudeBridgeConsoleView', 'claudeBridgeAgentsView', 'claudeBridgePlanView',
        'claudeBridgeTodosView', 'claudeBridgeToolsUsageView') { $ids.Add($id) }
    $toolsRaw = Invoke-Probe @{ cmd = 'tools' }
    if ($toolsRaw) {
        try {
            foreach ($t in ($toolsRaw | ConvertFrom-Json).tools) {
                foreach ($v in $t.views) { if ($v.id -and -not $ids.Contains($v.id)) { $ids.Add($v.id) } }
            }
        } catch {}
    }
    $log = Find-CefConsoleLog
    $offset = if ($log) { Get-FileLength $log } else { 0 }
    $sent = New-Object System.Collections.Generic.List[string]
    foreach ($id in $ids) {
        $js = $PageProbeJs.Replace('__ID__', $id).Replace('__NONCE__', $Nonce)
        $r = Invoke-Probe @{ cmd = 'wvjs'; id = $id; js = $js }
        $sent.Add("> $id`r`n< $r")
    }
    Start-Sleep -Seconds 4
    $results = @()
    $raw = ''
    if ($log) {
        $raw = Read-NewText $log $offset
        foreach ($line in ($raw -split "`r?`n")) {
            $i = $line.IndexOf('[kdiag] ')
            if ($i -lt 0 -or $line.IndexOf($Nonce) -lt 0) { continue }
            $body = $line.Substring($i + 8)
            $j = $body.LastIndexOf('", source:')
            if ($j -ge 0) { $body = $body.Substring(0, $j) }
            try { $results += ($body | ConvertFrom-Json) } catch { $results += [pscustomobject]@{ id = '?'; err = 'parse'; raw = $body } }
        }
    }
    return [pscustomobject]@{ Ids = $ids; Sent = $sent; Log = $log; Raw = $raw; Results = $results }
}

# --- разбор diag.log ---------------------------------------------------------

function Get-DiagStats([string]$Text) {
    $rows = @()
    foreach ($line in ($Text -split "`r?`n")) {
        if ($line -notmatch '^\[cef\] за секунду: кадров (\d+), кадров окна (\d+), отрисовок (\d+), заказов (\d+)') { continue }
        $row = [ordered]@{ Cef = [int]$Matches[1]; WinDraws = [int]$Matches[2]; Paints = [int]$Matches[3]; Requests = [int]$Matches[4]; GpuiFrames = 0; SceneMs = 0; PresentMs = 0 }
        if ($line -match 'окно сложило (\d+) кадров: сцена (\d+) мс, презент (\d+) мс') {
            $row.GpuiFrames = [int]$Matches[1]; $row.SceneMs = [int]$Matches[2]; $row.PresentMs = [int]$Matches[3]
        }
        $rows += [pscustomobject]$row
    }
    return $rows
}

# Среднее за секунду окна: секунды полного простоя строк в diag.log не дают.
function Avg($rows, [string]$Field, [double]$Seconds) {
    if (-not $rows -or $rows.Count -eq 0) { return 0 }
    $den = [Math]::Max($rows.Count, $Seconds)
    return [Math]::Round((($rows | Measure-Object -Property $Field -Sum).Sum) / $den, 1)
}

# ============================================================================

Write-Host "Папка пакета: $Out" -ForegroundColor Cyan
if ($RestartWith) {
    if (-not (Restart-Kamin $RestartWith)) { Say '!! Перезапуск не удался, меряю как есть.' }
}

$main = Get-MainProc
if (-not $main) {
    Say '!! Главный процесс kaminide-gpui.exe не найден — запустите KaminIDE и повторите.'
    Save 'summary.txt' $Summary
    exit 1
}
$mainId = [int]$main.ProcessId
Say "Главный процесс: PID $mainId  $($main.ExecutablePath)"

Save 'env.txt' (Get-EnvReport)

# Модули: какой D3D реально загружен в главный процесс.
$modules = @()
try { $modules = (Get-Process -Id $mainId).Modules } catch { Say "!! Модули главного процесса не прочитаны: $($_.Exception.Message) (нужен 64-битный PowerShell)" }
$gfxModules = $modules | Where-Object { $_.ModuleName -match 'd3d|dxgi|warp|nvwgf|nvd3d|atidxx|amdxx|igd|igc|libcef|vulkan|opengl' }
Save 'modules.txt' ($gfxModules | ForEach-Object { "{0,-28} {1}" -f $_.ModuleName, $_.FileName })
$warp = [bool]($modules | Where-Object { $_.ModuleName -ieq 'd3d10warp.dll' })

$procs = Get-KaminProcs
$procLines = $procs | ForEach-Object { "PID {0,-6} {1,-18} WS {2,6} MB  {3}" -f $_.ProcessId, (Get-ProcRole $_), [Math]::Round($_.WorkingSetSize / 1MB), $_.CommandLine }
Save 'processes.txt' $procLines

$diagPath = Join-Path $CacheDir 'diag.log'
$diagOffset = Get-FileLength $diagPath

Write-Host ''
Write-Host "Замер $SampleSeconds с. НЕ ТРОГАЙТЕ мышь и клавиатуру, окно KaminIDE оставьте как есть." -ForegroundColor Yellow
$cpu0 = Get-CpuSnapshot
$thr0 = Get-ThreadSnapshot $mainId
$t0 = Get-Date
$gpuSamples = @()
$sysLoad = @()
$steps = [Math]::Max(1, [Math]::Floor($SampleSeconds / 5))
for ($s = 0; $s -lt $steps; $s++) {
    # Сон до отметки, а не на фиксированный срок: WMI-запрос GPU сам идёт секунды.
    $mark = $t0.AddSeconds(($s + 1) * $SampleSeconds / $steps - 1)
    $wait = ($mark - (Get-Date)).TotalMilliseconds
    if ($wait -gt 0) { Start-Sleep -Milliseconds ([int]$wait) }
    $g = Get-GpuSample
    if ($g) { $gpuSamples += $g }
    try { $sysLoad += (Get-CimInstance Win32_Processor | Measure-Object -Property LoadPercentage -Average).Average } catch {}
    Write-Host "  ... $([int]((Get-Date) - $t0).TotalSeconds) с"
}
$elapsedMs = ((Get-Date) - $t0).TotalMilliseconds
$cpu1 = Get-CpuSnapshot
$thr1 = Get-ThreadSnapshot $mainId
# Сразу по концу окна: расчёты ниже идут секунды, и их строки исказили бы средние.
$diagWindow = Read-NewText $diagPath $diagOffset
$cores = [Environment]::ProcessorCount
Write-Host 'Замер окончен, можно трогать.' -ForegroundColor Green

# CPU по процессам (в «ядрах»: 1.0 = одно логическое ядро целиком).
$roleById = @{}
foreach ($p in $procs) { $roleById[[int]$p.ProcessId] = Get-ProcRole $p }
$cpuRows = foreach ($id in $cpu1.Keys) {
    if (-not $cpu0.ContainsKey($id)) { continue }
    $d = $cpu1[$id].Ms - $cpu0[$id].Ms
    if ($d -le 0) { continue }
    [pscustomobject]@{ Pid = $id; Name = $cpu1[$id].Name; Role = $roleById[$id]; Cores = [Math]::Round($d / $elapsedMs, 3) }
}
$cpuRows = $cpuRows | Sort-Object Cores -Descending
$kaminCores = [Math]::Round((($cpuRows | Where-Object { $_.Role } | Measure-Object -Property Cores -Sum).Sum), 2)
$mainCores = [double](($cpuRows | Where-Object { $_.Pid -eq $mainId } | Select-Object -First 1).Cores)
$cpuText = @("Окно $([Math]::Round($elapsedMs / 1000, 1)) с, логических ядер $cores, средняя загрузка CPU системы: $([Math]::Round((($sysLoad | Measure-Object -Average).Average), 0))%", '',
    'Ядер  Процесс (PID, роль)') + ($cpuRows | Select-Object -First 30 | ForEach-Object { "{0,6} {1} (PID {2}, {3})" -f $_.Cores, $_.Name, $_.Pid, $_.Role })
Save 'cpu.txt' $cpuText

# Потоки главного процесса с модулем стартовой функции.
$modRanges = $modules | ForEach-Object { [pscustomobject]@{ Name = $_.ModuleName; Base = $_.BaseAddress.ToInt64(); End = $_.BaseAddress.ToInt64() + $_.ModuleMemorySize } }
$thrRows = foreach ($tid in $thr1.Keys) {
    if (-not $thr0.ContainsKey($tid)) { continue }
    $d = $thr1[$tid].Ms - $thr0[$tid].Ms
    if ($d -le 0) { continue }
    $start = $thr1[$tid].Start
    $mod = ($modRanges | Where-Object { $start -ge $_.Base -and $start -lt $_.End } | Select-Object -First 1).Name
    [pscustomobject]@{ Tid = $tid; Cores = [Math]::Round($d / $elapsedMs, 3); Module = $mod; Name = (Get-ThreadName $tid) }
}
$thrRows = $thrRows | Sort-Object Cores -Descending
$byModule = $thrRows | Group-Object Module | ForEach-Object {
    [pscustomobject]@{ Module = $(if ($_.Name) { $_.Name } else { '?' }); Threads = $_.Count; Cores = [Math]::Round((($_.Group | Measure-Object -Property Cores -Sum).Sum), 3) }
} | Sort-Object Cores -Descending
Save 'threads.txt' (@('По модулю стартовой функции потока:') + ($byModule | ForEach-Object { "{0,6} ядер  {1,-28} потоков {2}" -f $_.Cores, $_.Module, $_.Threads }) +
    @('', 'Топ потоков:') + ($thrRows | Select-Object -First 40 | ForEach-Object { "{0,6} ядер  TID {1,-6} {2,-28} {3}" -f $_.Cores, $_.Tid, $_.Module, $_.Name }))

# GPU 3D по процессам.
$gpuByPid = $gpuSamples | Group-Object Pid | ForEach-Object {
    $id = [int]$_.Name
    $name = $cpu1[$id].Name
    [pscustomobject]@{ Pid = $id; Name = $name; Role = $roleById[$id]; AvgUtil = [Math]::Round((($_.Group | Measure-Object -Property Util -Sum).Sum) / $steps, 1) }
} | Sort-Object AvgUtil -Descending
Save 'gpu.txt' (@("GPU 3D, средний % за окно ($steps выборок):") + ($gpuByPid | ForEach-Object { "{0,6}%  {1} (PID {2}, {3})" -f $_.AvgUtil, $_.Name, $_.Pid, $_.Role }))

# diag.log за окно простоя.
Save 'diag-window.txt' $diagWindow
Copy-Tail $diagPath 'cache-diag.log'
$stats = Get-DiagStats $diagWindow

# Зонд страниц — ПОСЛЕ окна: его rAF-счётчик сам порождает кадры.
Write-Host 'Опрашиваю страницы CEF...'
$probeInfo = @()
foreach ($c in 'ping', 'tools', 'tree', 'overlay', 'focus') { $probeInfo += "> $c"; $probeInfo += "< $(Invoke-Probe @{ cmd = $c })" }
Save 'probe.txt' $probeInfo
$page = Invoke-PageProbe
$animText = @("CEF console log: $($page.Log)", '') + $page.Sent + @('', '--- результаты [kdiag] ---')
foreach ($r in $page.Results) { $animText += ($r | ConvertTo-Json -Depth 6 -Compress) }
Save 'anim.txt' $animText

if (-not $NoScreenshot) {
    $shot = Join-Path $Out 'window.png'
    Invoke-Probe @{ cmd = 'screenshot'; path = $shot } 15000 | Out-Null
}

# Логи.
Copy-Tail (Join-Path $CacheDir 'crash.log') 'cache-crash.log' 2MB
if ($page.Log) { Copy-Tail $page.Log 'cef-console.log' 3MB }
if (Test-Path $DataDir) {
    Get-ChildItem $DataDir -Filter '*.log' -File -ErrorAction SilentlyContinue | ForEach-Object { Copy-Tail $_.FullName ("data-" + $_.Name) 3MB }
}

# --- сводка -----------------------------------------------------------------

Say ''
Say "=== Сводка ($Label) ==="
Say "Растеризация окна: $(if ($warp) { 'ПРОГРАММНАЯ (d3d10warp.dll загружен)' } else { 'аппаратная (WARP не загружен)' })"
Say "CPU: KaminIDE всего $kaminCores ядер из $cores; главный процесс $mainCores"
$warpCores = ($byModule | Where-Object { $_.Module -ieq 'd3d10warp.dll' } | Select-Object -First 1).Cores
if ($warpCores) { Say "  из них потоки WARP (программный D3D): $warpCores ядер" }
$mainGpu = [double](($gpuByPid | Where-Object { $_.Pid -eq $mainId } | Select-Object -First 1).AvgUtil)
if ($gpuByPid) { Say "GPU 3D: главный процесс $mainGpu%; всего по KaminIDE $(( $gpuByPid | Where-Object { $_.Role } | Measure-Object -Property AvgUtil -Sum).Sum)%" }

if ($stats.Count -gt 0) {
    $sec = [Math]::Round($elapsedMs / 1000)
    $aCef = Avg $stats 'Cef' $sec; $aDraw = Avg $stats 'WinDraws' $sec; $aGf = Avg $stats 'GpuiFrames' $sec
    Say "diag.log ($($stats.Count) с с активностью из $SampleSeconds): кадров Chromium $aCef/с, перерисовок окна $aDraw/с, кадров gpui $aGf/с, сцена $(Avg $stats 'SceneMs' $sec) мс/с, презент $(Avg $stats 'PresentMs' $sec) мс/с"
    if ($aDraw -ge 10 -and $aCef -lt 2) {
        Say '  => Окно перерисовывается без кадров Chromium: кадры заказывает НАТИВНАЯ анимация gpui'
        Say '     (брендовый лоадер, спиннер, мигающая каретка терминала, пульс статуса сессии, прогресс тоста).'
    } elseif ($aCef -ge 10) {
        Say '  => Кадры гонит страница CEF — смотрите таблицу анимаций ниже и anim.txt.'
    } elseif ($aDraw -lt 2) {
        Say '  => Окно в простое почти не перерисовывается.'
    }
} else {
    Say "diag.log: за окно строк '[cef] за секунду' нет (полный простой или лог не пишется: $diagPath)"
}

if ($page.Results.Count -eq 0) {
    Say "Зонд страниц: ответов нет (лог консоли CEF: $(if ($page.Log) { $page.Log } else { 'не найден' }))"
} else {
    Say 'Страницы CEF (видимость | reduced-motion | наше глушение | rAF/с | анимаций):'
    foreach ($r in $page.Results) {
        if ($r.err) { Say "  $($r.id): ошибка $($r.err)"; continue }
        Say ("  {0,-28} {1,-8} rm={2,-5} mute={3,-5} raf={4,-3} n={5}" -f $r.id, $r.vis, $r.reducedMotion, $r.injectedMute, $r.rafPerSec, $r.nAnim)
        foreach ($a in $r.anims) {
            if ($a.state -ne 'running') { continue }
            Say ("      {0} [{1}, iter={2}, видим={3}, в кадре={4}, opacity={5}] {6}" -f $a.name, $a.state, $a.iter, $a.visible, $a.inViewport, $a.opacity, $a.el)
        }
    }
}

Save 'summary.txt' $Summary

if (-not $NoZip) {
    $zip = "$Out.zip"
    try {
        Compress-Archive -Path (Join-Path $Out '*') -DestinationPath $zip -Force
        Write-Host ''
        Write-Host "Готово: $zip" -ForegroundColor Green
    } catch {
        Write-Host "Архив не собран ($($_.Exception.Message)); файлы в $Out" -ForegroundColor Yellow
    }
} else {
    Write-Host "Готово: $Out" -ForegroundColor Green
}
Write-Host 'В логах и на скриншоте может быть содержимое чатов и пути — пакет только в private evidence repo.'
