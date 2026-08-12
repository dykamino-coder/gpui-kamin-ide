import "reflect-metadata";
import { afterEach, describe, expect, it } from "vitest";
import fs from "fs";
import os from "os";
import path from "path";
import {
  buildSessionClaudeArgs, cliDebugLogPath,
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

describe("cliDebugArgs", () => {
  const before = process.env.BRIDGE_CLI_DEBUG_LOG;
  afterEach(() => {
    if (before === undefined) delete process.env.BRIDGE_CLI_DEBUG_LOG;
    else process.env.BRIDGE_CLI_DEBUG_LOG = before;
  });

  it("stays out of the arguments unless asked for", () => {
    delete process.env.BRIDGE_CLI_DEBUG_LOG;
    const args = buildSessionClaudeArgs({ cwd: "/repo" } as never, "/tmp/session");
    expect(args).not.toContain("--debug-file");
  });

  it("points the CLI at a log inside the session directory", () => {
    process.env.BRIDGE_CLI_DEBUG_LOG = "1";
    const args = buildSessionClaudeArgs({ cwd: "/repo" } as never, "/tmp/session");
    const at = args.indexOf("--debug-file");
    expect(at).toBeGreaterThanOrEqual(0);
    expect(args[at + 1]).toBe(cliDebugLogPath("/tmp/session"));
  });
});
