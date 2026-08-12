// ============================================================================
// PTY Session — materialized plugin roots and Claude CLI launch arguments
// ============================================================================

import fs from "fs";
import path from "path";
import { buildClaudeArgs } from "./session-env";
import type { SessionConfig } from "./types";

/** Absolute proxy-plugin roots to pass as repeated `--plugin-dir` arguments. */
export function getSessionPluginDirs(settingsDir: string): string[] {
  const root = path.join(settingsDir, ".bridge-plugins");
  if (!fs.existsSync(root)) return [];
  try {
    return fs
      .readdirSync(root, { withFileTypes: true })
      .filter(
        (entry) =>
          entry.isDirectory() &&
          fs.existsSync(
            path.join(root, entry.name, ".claude-plugin", "plugin.json"),
          ),
      )
      .map((entry) => path.join(root, entry.name))
      .sort();
  } catch {
    return [];
  }
}

/** Keep plugin discovery wired to the actual spawn path, not only to the
 * lower-level argument builder used by unit tests. */
export function buildSessionClaudeArgs(
  config: SessionConfig,
  settingsDir: string,
): string[] {
  return buildClaudeArgs(config, getSessionPluginDirs(settingsDir));
}
