import fs from "fs";
import os from "os";
import path from "path";
import { afterEach, describe, expect, it } from "vitest";
import { prepareSyncStorage } from "./storage";

const tempDirs: string[] = [];

function tempDir(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "bridge-sync-storage-"));
  tempDirs.push(dir);
  return dir;
}

afterEach(() => {
  for (const dir of tempDirs.splice(0))
    fs.rmSync(dir, { recursive: true, force: true });
});

describe("sync storage", () => {
  it("migrates the legacy snapshot when the persistent target is absent", () => {
    const root = tempDir();
    const legacy = path.join(root, "legacy");
    const target = path.join(root, "data", "bridge-sync");
    fs.mkdirSync(path.join(legacy, "users", "abc"), { recursive: true });
    fs.writeFileSync(path.join(legacy, "users", "abc", "settings.json"), "{}");

    expect(prepareSyncStorage(target, legacy)).toBe(target);
    expect(
      fs.readFileSync(
        path.join(target, "users", "abc", "settings.json"),
        "utf-8",
      ),
    ).toBe("{}");
  });

  it("never overwrites an existing persistent snapshot with legacy data", () => {
    const root = tempDir();
    const legacy = path.join(root, "legacy");
    const target = path.join(root, "data", "bridge-sync");
    fs.mkdirSync(legacy, { recursive: true });
    fs.mkdirSync(target, { recursive: true });
    fs.writeFileSync(path.join(legacy, "marker.txt"), "legacy");
    fs.writeFileSync(path.join(target, "marker.txt"), "current");

    prepareSyncStorage(target, legacy);

    expect(fs.readFileSync(path.join(target, "marker.txt"), "utf-8")).toBe(
      "current",
    );
  });
});
