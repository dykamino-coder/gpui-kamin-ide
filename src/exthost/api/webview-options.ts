import type { URI } from "vscode-uri"
import type { CreateWebviewOptions, WebviewOptions, WebviewPanelOptions } from "./webview.js"

/** Split the `createWebviewPanel` options intersection into the two d.ts slices,
 *  copying only PRESENT keys (exactOptionalPropertyTypes rejects explicit
 *  undefined). The resolved roots are always exposed on `webviewOptions` so the
 *  extension reads back what `asWebviewUri` is actually confined to. */
export function splitCreateOptions(options: CreateWebviewOptions, rootUris: URI[]): { webviewOptions: WebviewOptions; panelOptions: WebviewPanelOptions } {
  const webviewOptions: WebviewOptions = { localResourceRoots: rootUris }
  if (options.enableScripts !== undefined) webviewOptions.enableScripts = options.enableScripts
  if (options.enableForms !== undefined) webviewOptions.enableForms = options.enableForms
  if (options.enableCommandUris !== undefined) webviewOptions.enableCommandUris = options.enableCommandUris
  if (options.portMapping !== undefined) webviewOptions.portMapping = options.portMapping
  const panelOptions: WebviewPanelOptions = {}
  if (options.enableFindWidget !== undefined) panelOptions.enableFindWidget = options.enableFindWidget
  if (options.retainContextWhenHidden !== undefined) panelOptions.retainContextWhenHidden = options.retainContextWhenHidden
  return { webviewOptions, panelOptions }
}
