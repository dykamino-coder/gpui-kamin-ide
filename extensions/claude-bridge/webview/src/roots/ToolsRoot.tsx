import { Fragment } from 'preact'
import type { JSX } from 'preact'
import { useRef } from 'preact/hooks'
import { useSignal } from '@preact/signals'

import { activePlan } from '../signals/plan'
import { activeTodos } from '../signals/todos'
import { tabs } from '../signals/tabs'
import { initializing } from '../signals/ui'
import { RestoringContent } from '../components/layout/EmptyState'

import { useBridge } from '../hooks/useBridge'
import { useInit } from '../hooks/useInit'
import { useBridgeListeners } from '../hooks/useBridgeListeners'
import { useThemeHotkey } from '../hooks/useThemeHotkey'

import { AgentsToolPanel } from '../components/agent-tiles/AgentsToolPanel'
import { ToolsUsagePanel } from '../components/tool-usage/ToolsUsagePanel'
import { PlanList } from '../components/right-panel/PlanList'
import { TodoList } from '../components/right-panel/TodoList'
import { TerminalPanel } from '../components/terminal/TerminalPanel'
import { Tooltip } from '../components/overlays/Tooltip'
import { ConfirmModalHost } from '../components/overlays/ConfirmModalHost'

// Plan / Todos / Agents / Console are separate KaminIDE tools (each its own
// auxiliarybar container + webview view, independently pinnable). They all load
// this one bundle; it reads WHICH section from its iframe URL (the view id is the
// path) and renders only that section — own iframe → own bridge listeners.
type Section = 'plan' | 'todos' | 'agents' | 'console' | 'toolsUsage'

function sectionFromUrl(): Section {
  const viewId = window.location.pathname.replace(/^\//, '')
  if (/Todos/i.test(viewId)) return 'todos'
  if (/Agents/i.test(viewId)) return 'agents'
  if (/Console/i.test(viewId)) return 'console'
  if (/ToolsUsage/i.test(viewId)) return 'toolsUsage'
  return 'plan'
}

function EmptyState({ icon, title, hint }: { icon: string; title: string; hint: string }): JSX.Element {
  return (
    <div style="display:flex;flex-direction:column;align-items:center;justify-content:center;flex:1;gap:8px;color:var(--text-disabled);text-align:center;padding:24px">
      <i class={`fas ${icon}`} style="font-size:26px;color:var(--text-disabled)" />
      <span style="font-size:13px;font-weight:500;color:var(--text-muted)">{title}</span>
      <span style="font-size:12px">{hint}</span>
    </div>
  )
}

export function ToolsRoot(): JSX.Element {
  const bridge = useBridge()
  const promptDebounceTimers = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map())
  const vscodeAvailable = useSignal(false)

  // Only the Agents section renders agent tiles; console/plan/todos must NOT
  // accumulate subagent transcripts + the agent tree (that copy was a big part
  // of the shared-heap OOM). Gate agent-data on the section, computed up front.
  const section = sectionFromUrl()
  useBridgeListeners(bridge, promptDebounceTimers, vscodeAvailable, 'tools', section === 'agents')
  useThemeHotkey()
  useInit(bridge, 'tools')
  const hasPlan = !!activePlan.value?.total
  const hasTodos = !!activeTodos.value?.total

  // Console = the raw Claude CLI terminal for the active session, wired to the
  // same PTY broadcast as the chat. Full-bleed (no padding/overflow wrapper) so
  // xterm owns its own scroll + fit.
  if (section === 'console') {
    // Paint the whole tool surface the editor/console colour so it matches the
    // native terminal (the KaminIDE card frame has no bg of its own — a webview
    // owns its backdrop). `fill` makes xterm span the full width, no 800px cap.
    // Its OWN loading state: with no tabs yet (app boot / session restore) the
    // TerminalPanel renders nothing, so the console read as a dead black panel
    // while the chat showed "Restoring…". Mirror that here so the console has a
    // loading state of its own; once a tab exists, TerminalContainer's own
    // "Connecting…" spinner covers the pre-output gap.
    const noTabs = tabs.value.length === 0
    return (
      <Fragment>
        <div style="display:flex;flex-direction:column;height:100%;width:100%;box-sizing:border-box;background:var(--editor-bg, #1d1c25)">
          {noTabs
            ? (initializing.value
              ? <RestoringContent />
              : <EmptyState icon="fa-terminal" title="Console" hint="The active session's terminal appears here." />)
            : <TerminalPanel visible={true} showResizeHandle={false} fill={true} />}
        </div>
        <Tooltip />
        <ConfirmModalHost />
      </Fragment>
    )
  }

  return (
    <Fragment>
      <div style="display:flex;flex-direction:column;height:100%;width:100%;overflow:auto;padding:8px;gap:10px;box-sizing:border-box">
        {section === 'agents' && <AgentsToolPanel />}
        {section === 'toolsUsage' && <ToolsUsagePanel />}
        {section === 'plan' && (hasPlan
          ? <PlanList />
          : <EmptyState icon="fa-list-check" title="Plan" hint="The active session's plan appears here." />)}
        {section === 'todos' && (hasTodos
          ? <TodoList />
          : <EmptyState icon="fa-square-check" title="Todos" hint="The active session's todos appear here." />)}
      </div>
      <Tooltip />
      <ConfirmModalHost />
    </Fragment>
  )
}
