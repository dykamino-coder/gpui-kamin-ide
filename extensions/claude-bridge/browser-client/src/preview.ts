// @kaminide/bridge-browser-client/preview: Module Federation preview rendering
// with host-owned hot updates. React 19 and the federation runtime are injected.
export { createPreviewRenderer, type FederationRuntime, type MountedPreview, type PreviewReact, type PreviewRendererOptions, type RenderRemote } from './preview/renderer'
export type { HostBase } from './address'
export { createPreviewView, type PreviewStatus, type PreviewView, type PreviewViewOptions } from './preview/view'
export type { RefreshRuntime } from './preview/hot'
export type { PreviewDescriptor, PreviewState } from './types'
