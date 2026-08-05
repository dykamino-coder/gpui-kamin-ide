# Claude Bridge terminology

KaminIDE is not an Electron application. The current desktop runtime is the
native Rust/GPUI shell with offscreen CEF, a Node `kamin-host` parent, and a
forked extension-host child.

Inside `extension/`, `@kaminide/host-compat` is a local VSIX facade over
webview postMessage and `vscode` APIs. Electron-shaped members such as
`ipcMain`, `BrowserWindow`, and `webContents` describe the compatibility
surface only; they are not imports from or evidence of an Electron runtime.

Use current-runtime names in new code and comments:

- native GPUI shell;
- CEF webview;
- Node `kamin-host`;
- extension host;
- VSIX bridge host or client host;
- Kamin bridge API (`window.kaminBridge`, `KaminBridgeApi`).

Use `Electron` only when referring to one of these compatibility boundaries:

- the removed standalone Electron Bridge client and its migration data;
- deprecated public aliases such as `window.electronBridge`, `ElectronBridge`,
  `ElectronToServerMsg`, and `ServerToElectronMsg`;
- legacy filesystem paths, download URLs, and uninstall identifiers that must
  remain stable for upgrades.

Do not rename those persisted or public compatibility identifiers without a
separate migration plan. Do not describe the current KaminIDE runtime as
Electron merely because a facade retains Electron-shaped member names.
