import fs from "fs";
import os from "os";
import path from "path";
import { debugLog, warnLog } from "../logging";
import { getDataDir } from "../stats/database/lifecycle";

/** The sync snapshot belongs on the same persistent volume as other server data. */
export function resolveSyncBase(): string {
  const configured = process.env.BRIDGE_SYNC_BASE?.trim();
  return path.resolve(configured || path.join(getDataDir(), "bridge-sync"));
}

export function resolveLegacySyncBase(): string {
  return path.resolve(os.homedir(), "bridge-sync");
}

/**
 * Prepare the sync store and copy the legacy container-layer store on first boot.
 * An existing target always wins so a stale legacy volume can never overwrite it.
 */
export function prepareSyncStorage(
  target = resolveSyncBase(),
  legacy = resolveLegacySyncBase(),
): string {
  if (fs.existsSync(target)) return target;

  try {
    if (target !== legacy && fs.existsSync(legacy)) {
      fs.mkdirSync(path.dirname(target), { recursive: true });
      fs.cpSync(legacy, target, { recursive: true });
      debugLog("[sync] Migrated legacy sync storage", {
        from: legacy,
        to: target,
      });
    } else {
      fs.mkdirSync(target, { recursive: true });
    }
  } catch (err) {
    warnLog("[sync] Failed to migrate legacy sync storage", {
      from: legacy,
      to: target,
      error: err instanceof Error ? err.message : String(err),
    });
    fs.mkdirSync(target, { recursive: true });
  }

  return target;
}
