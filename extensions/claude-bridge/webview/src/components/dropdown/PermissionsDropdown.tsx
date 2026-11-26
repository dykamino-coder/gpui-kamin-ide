import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import styles from './ModelDropdown.module.css'
import { Dropdown } from './Dropdown'
import { DropdownTrigger } from './DropdownTrigger'
import { DropdownOption } from './DropdownOption'
import { useBridge } from '../../hooks/useBridge'
import { activeTabId, tabs } from '../../signals/tabs'
import { jsonlEntriesByTab } from '../../signals/jsonl'
const CLAUDE_MODES: Record<string, string> = {
  default: 'Manual', manual: 'Manual', acceptEdits: 'Accept edits', plan: 'Plan', auto: 'Auto', dontAsk: "Don’t ask", bypassPermissions: 'Bypass permissions',
}

function lastClaudeMode(tabId: string, conversation?: string): string | undefined {
  const entries = jsonlEntriesByTab.value.get(tabId)
  if (!entries || !conversation) return
  for (let i = entries.length - 1; i >= 0; i--) {
    const row = entries[i] as { type?: string; mode?: string; permissionMode?: string; engine?: string; sessionId?: string; isSidechain?: boolean } | undefined
    if (!row || row.engine === 'codex' || row.isSidechain || row.sessionId !== conversation) continue
    const mode = row.permissionMode ?? (row.type === 'permission-mode' || row.type === 'mode' ? row.mode : undefined)
    if (mode && Object.hasOwn(CLAUDE_MODES, mode)) return CLAUDE_MODES[mode]
  }
}

export function PermissionsDropdown(): JSX.Element {
  const [openedFor, setOpenedFor] = useState<string | null>(null)
  const bridge = useBridge()
  const tabId = activeTabId.value
  const tab = tabs.value.find(t => t.id === tabId)
  const codex = tab?.engine === 'codex'
  const target = tab ? JSON.stringify([tab.id, tab.engine, tab.sessionId]) : null
  const open = target !== null && openedFor === target
  const collaboration = tab?.codexCollaboration
  const native = !codex && tab?.status === 'connected' ? tab.claudePermissions : undefined
  const selected = native?.selected && Object.hasOwn(CLAUDE_MODES, native.selected) ? native.selected : undefined
  const claude = selected ? CLAUDE_MODES[selected] : !native && tab && !codex ? lastClaudeMode(tab.id, tab.conversationId) : undefined
  const label = codex
    ? (collaboration?.selected === 'plan' ? 'Codex · Plan' : collaboration?.selected === 'default' ? 'Codex · Default' : 'Codex mode')
    : (claude ?? 'Claude permissions')
  const close = () => setOpenedFor(null)
  return (
    <Dropdown open={open} onToggle={() => setOpenedFor(open ? null : target)} align="left"
      trigger={<DropdownTrigger icon={codex ? 'fa-robot' : 'fa-shield-halved'} label={label} open={open} />}>
      {codex ? <>
        <div class={styles.groupLabel}>Codex · read-only server sandbox</div>
        {collaboration?.selected === 'plan' && !collaboration.available.includes('default') &&
          <DropdownOption icon="fa-comment" name="Reset to engine default"
            description="Clear the saved Plan override for the next turn." selected={false}
            disabled={!tab?.sessionId || tab.status !== 'connected'}
            onClick={() => { close(); if (tab?.sessionId) bridge.changeCodexCollaboration(tab.id, tab.sessionId, 'default') }} />}
        {collaboration?.available.map(mode =>
          <DropdownOption key={mode} icon={mode === 'plan' ? 'fa-clipboard-list' : 'fa-comment'}
            name={mode === 'plan' ? 'Plan' : 'Default'}
            description={mode === 'plan' ? 'Plan the task on the next turn.' : 'Work on the task on the next turn.'}
            selected={collaboration.selected === mode} disabled={!tab?.sessionId || tab.status !== 'connected'}
            onClick={() => { close(); if (tab?.sessionId) bridge.changeCodexCollaboration(tab.id, tab.sessionId, mode) }} />
        )}
      </> : <>
        <div class={styles.groupLabel}>Claude Code permission mode</div>
        {native?.available.filter(mode => Object.hasOwn(CLAUDE_MODES, mode)).map(mode =>
          <DropdownOption key={mode} icon="fa-shield-halved" name={CLAUDE_MODES[mode]!}
            description="Resume this idle Claude session with the selected permission mode."
            selected={selected === mode} disabled={!tab?.sessionId || selected === mode}
            onClick={() => { close(); if (tab?.sessionId) bridge.changeClaudePermissionMode(tab.id, tab.sessionId, mode) }} />
        )}
        {native?.requested && native.requested !== selected && Object.hasOwn(CLAUDE_MODES, native.requested) &&
          <div class={styles.notice}>Requested {CLAUDE_MODES[native.requested]}</div>}
        {native?.reason && <div class={styles.notice}>{native.reason}</div>}
        <DropdownOption icon="fa-terminal" name="Change mode in Console"
          description={(claude ? `Last reported: ${claude}. ` : '') + 'At the idle prompt, use Shift+Tab for Manual, Accept edits, Plan and any extra modes enabled by Claude Code.'}
          selected={false} disabled={!tab || tab.status !== 'connected'}
          onClick={() => { close(); if (tab) bridge.openPermissionConsole(tab.id) }} />
      </>}
    </Dropdown>
  )
}
