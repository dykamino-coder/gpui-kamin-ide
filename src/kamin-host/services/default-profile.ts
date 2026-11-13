import { homedir, tmpdir } from "node:os"
import path from "node:path"

/** Match the shell's default data root without trusting a redirected LOCALAPPDATA.
 * A profile launched with its own LOCALAPPDATA must never import home state. */
export function isDefaultDataDir(dataDir: string, platform = process.platform, home = homedir(), temp = tmpdir()): boolean {
  const paths = platform === "win32" ? path.win32 : path.posix
  const base = platform === "win32" ? paths.join(home, "AppData", "Local") : temp
  const expected = paths.resolve(base, "kaminide-gpui-dev", "data")
  const actual = paths.resolve(dataDir)
  return platform === "win32" ? actual.toLowerCase() === expected.toLowerCase() : actual === expected
}
