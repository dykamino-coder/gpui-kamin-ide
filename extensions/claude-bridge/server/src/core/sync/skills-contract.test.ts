import "reflect-metadata";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import fs from "fs";
import path from "path";

const mocked = vi.hoisted(() => ({
  home: `${(process.env.TMPDIR || "/tmp").replace(/\/$/, "")}/bridge-sync-contract-${process.pid}-${Date.now()}`,
}));

vi.mock("os", async (importOriginal) => {
  const actual = await importOriginal<typeof import("os")>();
  const replacement = { ...actual, homedir: () => mocked.home };
  return { ...replacement, default: replacement };
});

vi.mock("../auth/tokens", () => ({
  resolveToken: vi.fn(async (token: string) =>
    token === "owner-secret"
      ? { tokenId: "owner-id", userName: "owner" }
      : null,
  ),
}));

vi.mock("../pty/session-core", () => ({ getAllSessions: () => [] }));
vi.mock("../pty/session-settings", () => ({ refreshSessionSkills: vi.fn() }));

import {
  createSyncRoutes,
  getProjectSyncDir,
  getUserSyncDir,
  syncSkillsSnapshot,
  tokenHash,
} from "./routes";

const tempDirs: string[] = [];

function tempDir(): string {
  const dir = fs.mkdtempSync(
    path.join(process.env.TMPDIR || "/tmp", "bridge-sync-skills-"),
  );
  tempDirs.push(dir);
  return dir;
}

beforeEach(() => {
  fs.rmSync(mocked.home, { recursive: true, force: true });
});

function write(root: string, relativePath: string, content: string): void {
  const target = path.join(root, relativePath);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, content, "utf-8");
}

afterEach(() => {
  for (const dir of tempDirs.splice(0))
    fs.rmSync(dir, { recursive: true, force: true });
  fs.rmSync(mocked.home, { recursive: true, force: true });
});

describe("skills sync wire contract", () => {
  it("preserves the previous snapshot when an old partial client omits skills", async () => {
    const skillsDir = tempDir();
    write(skillsDir, "kept/SKILL.md", "old-client");

    expect(await syncSkillsSnapshot(skillsDir, undefined)).toEqual({
      present: false,
      changed: false,
      count: 0,
    });
    expect(
      fs.readFileSync(path.join(skillsDir, "kept/SKILL.md"), "utf-8"),
    ).toBe("old-client");
  });

  it("treats an explicit empty map as an exact empty snapshot", async () => {
    const skillsDir = tempDir();
    write(skillsDir, "removed/SKILL.md", "stale");

    expect(await syncSkillsSnapshot(skillsDir, {})).toEqual({
      present: true,
      changed: true,
      count: 0,
    });
    expect(fs.existsSync(skillsDir)).toBe(false);
  });

  it("removes paths missing from a present non-empty snapshot", async () => {
    const skillsDir = tempDir();
    write(skillsDir, "removed/SKILL.md", "stale");
    write(skillsDir, "kept/SKILL.md", "old");

    const update = await syncSkillsSnapshot(skillsDir, {
      "kept/SKILL.md": "new",
    });

    expect(update).toEqual({ present: true, changed: true, count: 1 });
    expect(fs.existsSync(path.join(skillsDir, "removed/SKILL.md"))).toBe(false);
    expect(
      fs.readFileSync(path.join(skillsDir, "kept/SKILL.md"), "utf-8"),
    ).toBe("new");
  });

  it("preserves omission and clears explicit empty snapshots through both routes", async () => {
    const app = createSyncRoutes();
    const hash = tokenHash("owner-secret");
    const headers = {
      Authorization: "Bearer owner-secret",
      "Content-Type": "application/json",
    };
    const post = (scope: "user" | "project", body: Record<string, unknown>) =>
      app.request(`/api/sync/${hash}/${scope}`, {
        method: "POST",
        headers,
        body: JSON.stringify(body),
      });

    expect(
      (await post("user", { skills: { "user/SKILL.md": "user" } })).status,
    ).toBe(200);
    expect((await post("user", {})).status).toBe(200);
    const userSkills = path.join(getUserSyncDir(hash), "skills");
    expect(
      fs.readFileSync(path.join(userSkills, "user/SKILL.md"), "utf-8"),
    ).toBe("user");
    expect((await post("user", { skills: {} })).status).toBe(200);
    expect(fs.existsSync(userSkills)).toBe(false);

    const projectPath = "/repo";
    expect(
      (
        await post("project", {
          projectPath,
          skills: { "project/SKILL.md": "project" },
        })
      ).status,
    ).toBe(200);
    expect((await post("project", { projectPath })).status).toBe(200);
    const projectSkills = path.join(
      getProjectSyncDir(hash, projectPath),
      "skills",
    );
    expect(
      fs.readFileSync(path.join(projectSkills, "project/SKILL.md"), "utf-8"),
    ).toBe("project");
    expect((await post("project", { projectPath, skills: {} })).status).toBe(
      200,
    );
    expect(fs.existsSync(projectSkills)).toBe(false);
  });
});
