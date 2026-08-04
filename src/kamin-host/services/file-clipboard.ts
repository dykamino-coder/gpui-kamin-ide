// Обмен файлами с системным клипбордом (CF_HDROP + Preferred DropEffect)
// через PowerShell/WinForms. Вынесено из file-io.ts: тот упёрся в потолок
// 250 строк (max-lines), а клипборд — самодостаточная пара функций.
import { execFile } from "node:child_process"
import { promisify } from "node:util"

const execFileAsync = promisify(execFile)

/** Put `paths` on the OS clipboard as a file drop (cut = move effect). */
export async function clipboardWriteFiles(paths: readonly string[], cut: boolean): Promise<void> {
  const script = [
    "Add-Type -AssemblyName System.Windows.Forms",
    "$col = New-Object System.Collections.Specialized.StringCollection",
    "foreach ($p in ($env:KAMIN_CLIP_PATHS -split \"\\n\")) { if ($p) { [void]$col.Add($p) } }",
    "if ($col.Count -eq 0) { return }",
    "$data = New-Object System.Windows.Forms.DataObject",
    "$data.SetFileDropList($col)",
    "$effect = if ($env:KAMIN_CLIP_CUT -eq '1') { 2 } else { 5 }",
    "$ms = New-Object System.IO.MemoryStream",
    "$ms.Write([byte[]]($effect,0,0,0),0,4)",
    "$data.SetData('Preferred DropEffect',$ms)",
    "[System.Windows.Forms.Clipboard]::SetDataObject($data,$true)",
  ].join("; ")
  await execFileAsync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Sta", "-Command", script], {
    windowsHide: true,
    env: { ...process.env, KAMIN_CLIP_PATHS: paths.join("\n"), KAMIN_CLIP_CUT: cut ? "1" : "0" },
  })
}

/** Read the OS clipboard's file drop list + whether it was a cut (move). */
export async function clipboardReadFiles(): Promise<{ paths: string[]; cut: boolean }> {
  const script = [
    "Add-Type -AssemblyName System.Windows.Forms",
    "$list = [System.Windows.Forms.Clipboard]::GetFileDropList()",
    "$effect = 5",
    "try { $d = [System.Windows.Forms.Clipboard]::GetData('Preferred DropEffect'); if ($d -is [System.IO.MemoryStream]) { $b = New-Object byte[] 4; [void]$d.Read($b,0,4); $effect = [int]$b[0] } } catch {}",
    "Write-Output ('EFFECT:' + $effect)",
    "foreach ($f in $list) { Write-Output $f }",
  ].join("; ")
  const { stdout } = await execFileAsync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Sta", "-Command", script], { windowsHide: true })
  const paths: string[] = []
  let cut = false
  for (const line of stdout.split(/\r?\n/)) {
    if (!line) continue
    if (line.startsWith("EFFECT:")) cut = (Number(line.slice("EFFECT:".length)) & 2) !== 0 // DROPEFFECT_MOVE
    else paths.push(line)
  }
  return { paths, cut }
}
