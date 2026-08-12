import "reflect-metadata";
import { afterEach, describe, expect, it } from "vitest";
import fs from "fs";
import os from "os";
import path from "path";
import {
  buildSessionClaudeArgs,
  getSessionPluginDirs,
} from "./session-plugin-args";

const tempDirs: string[] = [];

function tempDir(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "bridge-plugin-args-"));
  tempDirs.push(dir);
  return dir;
}

function materializePlugin(settingsDir: string, name: string): string {
  const root = path.join(settingsDir, ".bridge-plugins", name);
  fs.mkdirSync(path.join(root, ".claude-plugin"), { recursive: true });
  fs.writeFileSync(
    path.join(root, ".claude-plugin", "plugin.json"),
    "{}",
    "utf-8",
  );
  return root;
}

afterEach(() => {
  for (const dir of tempDirs.splice(0))
    fs.rmSync(dir, { recursive: true, force: true });
});

describe("session plugin launch args", () => {
  it("passes every valid materialized plugin root to the actual session args", () => {
    const settingsDir = tempDir();
    const beta = materializePlugin(settingsDir, "beta");
    const alpha = materializePlugin(settingsDir, "alpha");
    fs.mkdirSync(path.join(settingsDir, ".bridge-plugins", "incomplete"), {
      recursive: true,
    });

    expect(getSessionPluginDirs(settingsDir)).toEqual([alpha, beta]);

    const args = buildSessionClaudeArgs({ cwd: "/repo" }, settingsDir);
    expect(args.filter((arg) => arg === "--plugin-dir")).toHaveLength(2);
    expect(args).toContain(alpha);
    expect(args).toContain(beta);
    expect(args).not.toContain(
      path.join(settingsDir, ".bridge-plugins", "incomplete"),
    );
  });

  it("stays connected to the production session spawn path", () => {
    const sessionCore = fs.readFileSync(
      new URL("./session-core.ts", import.meta.url),
      "utf-8",
    );
    expect(sessionCore).toMatch(
      /buildSessionClaudeArgs\(config,\s*settingsDir\)/,
    );
  });
});
