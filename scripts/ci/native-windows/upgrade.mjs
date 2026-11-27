// Drive the published baseline's built-in updater against a private loopback feed serving the verified candidate.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { closeSync, createReadStream, openSync } from "node:fs";
import { stat, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { probe } from "./native-probe.mjs";
import { readRuntimeDiagnostics } from "./runtime-diagnostics.mjs";

const [exe, installer, before, after, output, capture] = process.argv.slice(2);
const size = (await stat(installer)).size;
const evidence = { before, after, feedRequests: 0, installerRequests: 0, checks: [] };
const archivePath = "/" + path.basename(installer);
const server = createServer((req, res) => {
  if (req.url === "/latest.json") {
    evidence.feedRequests++;
    res.writeHead(200, { "Content-Type": "application/json" });
    return res.end(JSON.stringify({ version: after, url: origin + archivePath }));
  }
  if (req.url === archivePath) {
    evidence.installerRequests++;
    res.writeHead(200, { "Content-Length": size, "Content-Type": "application/octet-stream" });
    const stream = createReadStream(installer);
    stream.once("error", () => res.destroy());
    res.once("close", () => stream.destroy());
    return stream.pipe(res);
  }
  res.writeHead(404).end();
});
await new Promise((resolve, reject) => server.once("error", reject).listen(0, "127.0.0.1", resolve));
const origin = "http://127.0.0.1:" + server.address().port;

async function boundary(name, check, timeout = 60000) {
  const entry = { name, status: "running" };
  evidence.checks.push(entry);
  const deadline = Date.now() + timeout;
  while (true) {
    try { await check(); entry.status = "passed"; return; }
    catch {
      if (Date.now() >= deadline) { entry.status = "failed"; throw new Error(name); }
      await delay(750);
    }
  }
}
try {
  const out = openSync(path.join(capture, "previous-stdout.log"), "w");
  const err = openSync(path.join(capture, "previous-stderr.log"), "w");
  const old = spawn(exe, ["--do-not-de-elevate"], { cwd: path.dirname(exe), stdio: ["ignore",out,err], detached: true,
    env: { ...process.env, KAMIN_GPUI_UPDATE_URL: origin + "/latest.json" } });
  await new Promise((resolve, reject) => { old.once("spawn", resolve); old.once("error", reject); });
  closeSync(out); closeSync(err);
  old.unref();
  await boundary("baseline-running-and-checking-feed", async () => {
    assert.equal((await probe({cmd:"ping"})).version, before);
    assert.ok(evidence.feedRequests > 0);
  });
  await boundary("update-pill-starts-installer-download", async () => {
    if (!evidence.installerRequests) {
      const {bounds} = await probe({cmd:"metric", id:"status-bar"});
      assert.ok(bounds.w > 100 && bounds.h > 5);
      await probe({cmd:"click", x:bounds.x + bounds.w - 40, y:bounds.y + bounds.h / 2});
      await delay(800);
    }
    assert.ok(evidence.installerRequests > 0);
  });
  await boundary("candidate-restarted-by-updater", async () => {
    assert.equal((await probe({cmd:"ping"})).version, after);
    assert.notEqual(old.exitCode ?? old.signalCode, null);
  }, 150000);
  evidence.status = "passed";
} catch (err) {
  evidence.status = "failed";
  console.error("Installed upgrade boundary failed: " + err.message);
  process.exitCode = 1;
} finally {
  server.close();
  server.closeAllConnections();
  evidence.runtime = await readRuntimeDiagnostics(capture);
  await writeFile(path.join(output, "upgrade.json"), JSON.stringify(evidence, null, 2) + "\n");
}
