// postMessage transport for the Bridge webview ↔ extension host. The core is
// shared with @kaminide/bridge-browser-client (browser-client/src/transport.ts).
// The port reads `vscodeApi` live: the browser entry installs its socket before
// importing the renderer, and tests replace the module.
import { vscodeApi } from "./webview-api.js"
import type { HostPort } from '../../../browser-client/src/socket'
import { createBridgeTransport } from '../../../browser-client/src/transport'

const port: HostPort = {
  postMessage: message => vscodeApi.postMessage(message),
  getState: () => vscodeApi.getState(),
  setState: state => vscodeApi.setState(state),
  get onHostMessage() { return vscodeApi.onHostMessage?.bind(vscodeApi) },
  get onDisconnect() { return vscodeApi.onDisconnect?.bind(vscodeApi) },
}
const transport = createBridgeTransport(port, typeof window === 'undefined' ? undefined : window)

export const inv = transport.inv
export const snd = transport.snd
export const sub = transport.sub
