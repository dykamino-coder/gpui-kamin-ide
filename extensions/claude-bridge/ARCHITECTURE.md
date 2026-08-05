# Claude Bridge architecture and naming

KaminIDE is not an Electron application. The shipped desktop shell is Rust/GPUI,
web content is rendered by offscreen CEF, and VSIX-compatible JavaScript runs in
the Node extension host.

Use these names in new code and reviews:

- **native GPUI shell** — the Rust/GPUI desktop application and its offscreen
  CEF renderer.
- **Node `kamin-host`** — the Node parent process started by the native shell;
  it owns host services and forks the extension-host child.
- **extension host** — the child Node process that loads VSIX-compatible
  extensions.
- **VSIX bridge host** or **client host** — `extension/src/bridge-host.ts` and
  the vendored client-side handlers that execute files, shell, hooks, MCP, LSP,
  and monitors on the user's machine.
- **bridge server** — the service that owns Claude CLI PTY sessions.
- **Kamin bridge API** — the webview-to-VSIX postMessage API. New code may use
  `window.kaminBridge` / `KaminBridgeApi`.

The remaining Electron names are compatibility boundaries, not runtime claims:

- imports from `electron` are aliased by `extension/build.mjs` to
  `extension/src/shim/electron.ts`;
- `window.electronBridge` and `ElectronBridge` are deprecated aliases retained
  for the vendored webview and third-party integrations;
- `ElectronToServerMsg`, old `electron-store` paths, `/download/electron`, and
  migration comments describe the legacy standalone Electron client or its
  persisted data and must not be blindly renamed.

A full removal of the deprecated webview alias is a separate breaking migration.
Do not infer Electron from structural names such as `BrowserWindow`, `ipcMain`,
or `webContents`: those interfaces are implemented locally by the VSIX shim.
