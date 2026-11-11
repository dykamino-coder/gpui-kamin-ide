// @kaminide/bridge-browser-client: framework-agnostic browser client of a Kamin
// Bridge workspace host. Preview rendering lives in the ./preview entry.
export { createBridgeClient, type BridgeClient, type BridgeClientOptions, type WorkspaceSnapshot } from './client'
export { createBrowserSocket, type BridgeSocket, type BridgeSocketLike, type BridgeSocketOptions, type ConnectionState, type HostPort } from './socket'
export { createBridgeTransport, type BridgeTransport, type Dispose, type EventCallback } from './transport'
export type { ConnectionConfig, CreateTabOptions, JsonlEntry, JsonlStatus, PreviewDescriptor, PreviewState, SessionConnectionState, Tab, TabId } from './types'
