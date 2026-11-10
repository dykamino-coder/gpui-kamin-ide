import { render } from 'preact'
import '@xterm/xterm/css/xterm.css'
import '@fortawesome/fontawesome-free/css/all.min.css'
import '../theme/global.css'
import '../theme/dark-theme.css'
import '../theme/light-theme.css'
import '../theme/components/chat.css'
import '../theme/components/jsonl.css'
import '../theme/components/widgets.css'
import '../theme/webview-overrides.css'
import '../theme/kamin-bridge.css'
import '../bridge-shim'
import { WorkspaceShell } from './WorkspaceShell'
import { applyInitialTheme } from '../theme/apply-theme'
import { installHljsThemeSwapper } from '../theme/highlight-theme'
import { notifyHostReady } from '../lib/host-ready'
import { installExternalLinkHandler } from '../lib/external-links'
import { installCommandApi } from '../lib/command-api'
import { connectDrafts } from './drafts'

export function mount(root: HTMLElement): void {
  connectDrafts()
  applyInitialTheme()
  installHljsThemeSwapper()
  installExternalLinkHandler()
  installCommandApi(window.kaminBridge)
  root.style.cssText = 'display:flex;height:100%;width:100%;overflow:hidden'
  render(<WorkspaceShell />, root)
  notifyHostReady()
}
