// React is deliberately separate from the Preact chat tree. The renderer itself
// is the shared one from @kaminide/bridge-browser-client; this UI injects its
// development React graph (preview-react.ts) and the federation runtime.
import { React, ReactDOM, ReactClient, JSXRuntime, JSXDevRuntime, Refresh, reactVersion } from './preview-react'
import { init, registerRemotes, loadRemote } from '@module-federation/enhanced/runtime'
import { createPreviewRenderer } from '../../../browser-client/src/preview/renderer'

export type { PreviewDescriptor as RemoteDescriptor } from '../../../browser-client/src/types'
export type { MountedPreview as Mounted } from '../../../browser-client/src/preview/renderer'

export const renderRemote = createPreviewRenderer({
  react: { React, ReactDOM, ReactClient, JSXRuntime, JSXDevRuntime, Refresh, version: reactVersion },
  federation: { init, registerRemotes, loadRemote },
})
