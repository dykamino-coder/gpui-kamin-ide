import { execFile } from "node:child_process";
import { promisify } from "node:util";

const run = promisify(execFile);

export async function probe(req) {
  try {
    const { stdout } = await run(process.execPath, ["scripts/probe.mjs", JSON.stringify(req)],
      { timeout: 12500, windowsHide: true });
    return JSON.parse(stdout);
  } catch (error) {
    const output = (error.stdout ?? "") + (error.stderr ?? "");
    // Map only known public diagnostics. The raw response can contain profile data.
    const reason = ["ECONNREFUSED", "view is not live", "probe: timeout", "ws not connected",
      "event channel not ready", "unknown region", "timeout"].find(value => output.includes(value));
    throw Object.assign(new Error("Probe call failed"), {reason: reason ?? "probe error"});
  }
}
