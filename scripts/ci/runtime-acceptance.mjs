#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const runtimeCode = /^(?:src\/|crates\/[^/]+\/src\/|extensions\/claude-bridge\/(?:server|extension|webview)\/src\/|builtin-extensions\/claude-bridge\/).+\.(?:rs|ts|tsx|js|mjs|html)$/;

function section(body, names) {
  return body.split(/^## /m).find((part) => names.test(part.split(/\r?\n/, 1)[0])) ?? "";
}

function field(body, name) {
  return body.match(new RegExp(`^- ${name}:\\s*(.+)$`, "mi"))?.[1].trim() ?? "";
}

export function primaryTask(body, title = "") {
  const task = section(body, /^(?:Task|Задача)$/i);
  const id = /\b(?:BR-\d+[A-Z]*|INC-\d{4}-\d{4})\b/;
  return task.match(id)?.[0] ?? title.match(id)?.[0] ??
    body.slice(0, 1000).match(id)?.[0] ?? null;
}

export function requiresWindowsRuntimeGate(card) {
  const explicit = card.match(/\*\*Windows runtime merge gate:\*\*\s*(required|not required)/i)?.[1];
  if (explicit) return explicit.toLowerCase() === "required";
  const acceptance = card.match(/\*\*Acceptance:\*\*([^\n]*(?:\n(?!\n)[^\n]*)?)/i)?.[1] ?? "";
  return /Windows/i.test(acceptance) &&
    /runtime|CEF|GPUI|Agent Teams|\bUI\b/i.test(acceptance) &&
    !/не требуется|not required|потребуется отдельному/i.test(acceptance);
}

export function validateRuntimeAcceptance({ body, title = "", files, headSha, mergeSha, card }) {
  if (!files.some((path) => runtimeCode.test(path))) return;
  const taskId = primaryTask(body, title);
  if (!taskId) return;
  if (!card) throw new Error(`Task card for ${taskId} is missing`);
  if (!requiresWindowsRuntimeGate(card)) return;
  const acceptance = section(body, /^Runtime acceptance$/i);
  if (!acceptance) {
    throw new Error(`${taskId} requires ## Runtime acceptance with real Windows evidence; a green CI job alone is not UI acceptance`);
  }
  const declaredTask = field(acceptance, "Task");
  if (declaredTask !== taskId) {
    throw new Error(`Runtime acceptance must name the primary task ${taskId}`);
  }
  const gate = field(acceptance, "Windows runtime merge gate").toLowerCase();
  if (gate !== "passed") {
    throw new Error(`${taskId} requires a Windows runtime merge gate before merge; CI compilation and tests do not satisfy it`);
  }
  const candidate = field(acceptance, "Candidate");
  if (candidate !== headSha && candidate !== mergeSha) {
    throw new Error("Windows runtime evidence must identify this exact PR head or merge candidate SHA");
  }
  if (field(acceptance, "Scenario").length < 20 ||
      field(acceptance, "Observed").length < 20) {
    throw new Error("Windows runtime evidence needs a concrete scenario and observed result");
  }
  const evidence = field(acceptance, "Evidence");
  if (!/^https:\/\/\S+/.test(evidence) || /\/actions\/runs\//.test(evidence)) {
    throw new Error("Link task-specific runtime evidence; a generic Actions run is not UI evidence");
  }
}

export async function taskCard(taskId) {
  if (taskId.startsWith("INC-")) {
    return readFile(`extensions/claude-bridge/runtime-issues/${taskId}.md`, "utf8");
  }
  const register = await readFile("extensions/claude-bridge/RUNTIME_RELIABILITY.md", "utf8");
  const start = register.search(new RegExp(`^### ${taskId}\\b`, "m"));
  if (start < 0) return "";
  const rest = register.slice(start);
  const next = rest.indexOf("\n### BR-", 1);
  return next < 0 ? rest : rest.slice(0, next);
}

if (process.argv[1]?.endsWith("/runtime-acceptance.mjs")) {
  try {
    const eventPath = process.argv[process.argv.indexOf("--event") + 1];
    const filesPath = process.argv[process.argv.indexOf("--files") + 1];
    if (!eventPath || !filesPath) throw new Error("Expected --event and --files");
    const event = JSON.parse(await readFile(eventPath, "utf8"));
    const body = event.pull_request?.body ?? "";
    const title = event.pull_request?.title ?? "";
    const files = (await readFile(filesPath, "utf8")).split(/\r?\n/).filter(Boolean);
    if (files.some((path) => runtimeCode.test(path))) {
      const taskId = primaryTask(body, title);
      validateRuntimeAcceptance({
        body,
        title,
        files,
        headSha: event.pull_request?.head?.sha,
        mergeSha: process.env.GITHUB_SHA,
        card: taskId ? await taskCard(taskId) : "",
      });
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
