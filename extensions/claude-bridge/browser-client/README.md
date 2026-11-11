# @kaminide/bridge-browser-client

Framework-agnostic browser client of a Kamin Bridge workspace host
([`@kaminide/bridge-workspace-host`](../workspace/HEADLESS.md) or the bundled
workspace container). It is the transport the KaminIDE browser UI itself uses:
the same `/bridge` socket, the same `invoke` / `send` / `event` frames and the
same Module Federation preview renderer. No UI framework, no runtime
dependencies; React and the federation runtime are injected only for preview
rendering.

The original request and its mapping to this package:
[STUDIO-CLIENT-SPEC.md](STUDIO-CLIENT-SPEC.md).

## Install

Each KaminIDE GitHub Release `kaminide-v<app version>` carries
`kaminide-bridge-browser-client-<version>.tgz`, where `<version>` is the Bridge
server version of that release and equals the version of the matching
`kaminide-bridge-workspace-host-<version>.tgz`. Use the same version on both
sides.

## Client

```ts
import { createBridgeClient } from '@kaminide/bridge-browser-client'

const client = createBridgeClient()          // same origin; or { origin: 'https://studio.example' }
await client.connect()                        // cookie session must already exist
const { active } = await client.sync()
const tab = active ?? await client.createTab()
client.onStreaming((tabId, entry) => { /* render the streaming message */ })
client.onJsonlEntries((tabId, entries) => { /* append transcript entries */ })
client.submitPrompt(tab, 'Build a landing page with a form')
```

| Member | Frame |
| --- | --- |
| `connect()` / `disconnect()` / `connectionState` / `onConnectionState()` | `WS /bridge` |
| `sync()` → `{ tabs, active }` | send `workspace:sync`, resolves at `workspace:snapshot-end` |
| `tabs()`, `activeTabId()`, `onTabsChanged()` | `workspace:snapshot-start`, `tab:list-changed`, `tab:switched` |
| `createTab({ cwd })` | invoke `tab:create` |
| `resumeSession(conversationId, { cwd, label })` | invoke `session:resume` |
| `switchTab(id)`, `closeTab(id)` | send `tab:switch`, `tab:close` |
| `submitPrompt(tab, text, submissionId?)` | send `pty-submit-text` |
| `interrupt(tab)` | send `session:interrupt` |
| `onJsonlEntries`, `onStreaming`, `onStatus` | events `jsonl-entries`, `streaming-entry`, `jsonl-status` |
| `invoke(channel, ...args)`, `send(...)`, `on(...)` | any channel of the KaminIDE bridge API |
| `preview.state()`, `preview.onState()` | event `workspace:preview-state` |
| `preview.fetchState()`, `preview.unmount()` | `GET /preview/state`, `POST /preview/unmount` |

Channel names and argument shapes for the raw methods are the tables in
`webview/src/lib/bridge-channels.ts` of the source repository. Types `Tab`,
`JsonlEntry`, `JsonlStatus`, `PreviewState` and others are exported from the
package; do not import engine paths.

Connection semantics are those of the KaminIDE browser UI:

- `connect()` relies on the workspace session cookie. Sign-in is done by the
  embedding site through the host's `POST /auth/login`; the client never sees
  credentials.
- Close code 4001 means the session ended (`authentication-required`), 4002 or
  4008 a protocol or size violation (`closed`). Both reject pending `connect()`
  calls; call `connect()` again after a new sign-in. Other losses reconnect with
  backoff (`offline` → `connecting` → `connected`).
- An `invoke` pending during a loss rejects with an unknown outcome: the host
  may already have executed it. Nothing is replayed. After a reconnect the
  client sends `workspace:sync` (option `syncOnReconnect`, default on) and the
  snapshot arrives through the same subscriptions.
- Subscriptions survive reconnects and `disconnect()` / `connect()` cycles.

## Preview

```ts
import { createPreviewRenderer, createPreviewView } from '@kaminide/bridge-browser-client/preview'
import * as React from 'react'
import * as ReactDOM from 'react-dom'
import * as ReactClient from 'react-dom/client'
import * as JSXRuntime from 'react/jsx-runtime'
import * as JSXDevRuntime from 'react/jsx-dev-runtime'
import * as Refresh from 'react-refresh/runtime'
import { init, registerRemotes, loadRemote } from '@module-federation/enhanced/runtime'

const render = createPreviewRenderer({
  react: { React, ReactDOM, ReactClient, JSXRuntime, JSXDevRuntime, Refresh, version: React.version },
  federation: { init, registerRemotes, loadRemote },
})
const view = createPreviewView({ states: listener => client.preview.onState(listener), render })
container.append(view.element)
view.onChange(() => console.log(view.status, view.descriptor?.name, view.error))
```

The view mounts the remote the agent published with `preview_mount`, applies
props updates in place, remounts on a new revision, follows the remote's own
HMR through `/preview/remotes/<name>/ws`, and remounts a non-HMR remote 5 s
after its entry was rebuilt. `status` is `empty`, `unavailable` (dev server
down), `mounting`, `ready` or `error`. `reload()` remounts; `dispose()`
unmounts and stops following.

Requirements for preview rendering: React 19 with `react-refresh/runtime`
injected into the global hook before `react-dom` loads (remotes share this
exact React singleton), and `@module-federation/enhanced` runtime. These are
optional peer dependencies; the bundled KaminIDE UI is tested with the versions
listed in the package's `peerDependencies`. Remotes are trusted same-origin
code.

## Responsibilities of the embedding page

- Serve the page from the workspace origin (or a reverse proxy that presents
  the host's exact `origin`): the socket, cookies and preview proxy are
  same-origin only.
- Establish the cookie session before `connect()`, and handle
  `authentication-required` by signing in again.
- Render everything: the package draws nothing except the remote preview into
  the element you place.

## Build from source

```sh
npm --prefix extensions/claude-bridge/browser-client ci
npm --prefix extensions/claude-bridge/browser-client test
npm --prefix extensions/claude-bridge/browser-client run build      # dist/*.js and generated dist/*.d.ts
npm --prefix extensions/claude-bridge/browser-client run pack:client
```
