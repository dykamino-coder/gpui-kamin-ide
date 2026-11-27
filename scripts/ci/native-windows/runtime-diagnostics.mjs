// Export only source line locations, error codes and recognised labels. Never export raw runtime logs.
import { open } from "node:fs/promises";
import path from "node:path";

async function tail(file) {
  const handle = await open(file, "r").catch(() => null);
  if (!handle) return "";
  try {
    const { size } = await handle.stat();
    const buf = Buffer.alloc(Math.min(size, 128 * 1024));
    const { bytesRead } = await handle.read(buf, 0, buf.length, size - buf.length);
    return buf.toString("utf8", 0, bytesRead);
  } finally { await handle.close(); }
}

export async function readRuntimeDiagnostics(capture) {
  const cache = path.join(capture, "LOCALAPPDATA", "kaminide-gpui-dev", "cache");
  const files = ["stderr.log", "stdout.log", "previous-stderr.log", "previous-stdout.log"].map(f => path.join(capture, f));
  files.push(...["diag.log", "host.log", "crash.log"].map(f => path.join(cache, f)));
  const logs = (await Promise.all(files.map(tail))).join("\n");
  return {
    cefInitialized: /\[cef\] initialize=1\b/.test(logs),
    microsoftSoftwareAdapterObserved: /Microsoft Basic Render Driver/i.test(logs),
    hostChildReadyLogged: /\bCHILD_READY\b/.test(logs),
    rustPanicLogged: /\bpanicked at\b/.test(logs),
    panicSources: [...new Set(Array.from(logs.matchAll(/\bpanicked at [^\r\n]*?\b([a-z_][a-z_0-9.-]{0,60}\.rs:\d{1,6}:\d{1,5})/ig), m => m[1]))],
    windowsErrors: [...new Set(Array.from(logs.matchAll(/\b(?:0x[89c][0-9a-f]{7}|DXGI_ERROR_[A-Z_]+)\b/ig), m=>m[0]))].slice(0,20),
    operations: ["CreateDXGIFactory2", "EnumWarpAdapter", "D3D11CreateDevice", "EnumOutputs", "DirectComposition",
      "OpenWindow", "GetDpiForMonitor", "CreateSwapChain", "DWriteCreateFactory", "Graphics Tools", "Embed fonts",
      "CreateWindowExW", "ActiveX", "CLSID_DragDropHelper", "UISettings", "E_ACCESSDENIED", "E_NOINTERFACE"].filter(x=>logs.toLowerCase().includes(x.toLowerCase())),
  };
}
