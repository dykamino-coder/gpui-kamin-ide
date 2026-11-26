import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import styles from './ModelDropdown.module.css'
import { Dropdown } from './Dropdown'
import { DropdownTrigger } from './DropdownTrigger'
import { DropdownOption } from './DropdownOption'
import { useBridge } from '../../hooks/useBridge'
import { activeTabId, tabs } from '../../signals/tabs'
import { jsonlEntriesByTab } from '../../signals/jsonl'
import type { WorkspacePermissionMode } from '../../../shared/types'

const WORKSPACE_OPTIONS = [
  { value: 'ask', icon: 'fa-shield-halved', name: 'Use approval rules', description: 'Ask for tools that need approval.' },
  { value: 'readOnly', icon: 'fa-book-open', name: 'Read tools only', description: 'Allow file reads and searches. Block edits, shell and external tools.' },
  { value: 'acceptEdits', icon: 'fa-code', name: 'Accept file edits', description: 'Allow file edits by default. Explicit tool rules still apply.' },
  { value: 'dontAsk', icon: 'fa-ban', name: 'Deny on prompt', description: 'Deny any tool that would require a prompt.' },
  { value: 'bypassPermissions', icon: 'fa-triangle-exclamation', name: 'Allow workspace tools', description: 'Skip the workspace host approval prompt.' },
] satisfies { value: WorkspacePermissionMode; icon: string; name: string; description: string }[]

const CLAUDE_MODES: Record<string, string> = {
  default: 'Default', acceptEdits: 'Accept edits', plan: 'Plan', auto: 'Auto', dontAsk: "Don’t ask", bypassPermissions: 'Bypass permissions',
}

function lastClaudeMode(tabId: string, conversation?: string): string | undefined {
  const entries = jsonlEntriesByTab.value.get(tabId)
  if (!entries) return
  for (let i = entries.length - 1; i >= 0; i--) {
    const row = entries[i] as { type?: string; mode?: string; permissionMode?: string; engine?: string; sessionId?: string } | undefined
    if (!row || row.engine === 'codex' || (conversation && row.sessionId && row.sessionId !== conversation)) continue
    const mode = row.type === 'mode' || row.type === 'permission-mode' ? row.mode : row.permissionMode
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
  const workspace = WORKSPACE_OPTIONS.find(o => o.value === tab?.workspacePermission)
  const collaboration = tab?.codexCollaboration
  const claude = tab && !codex ? lastClaudeMode(tab.id, tab.conversationId) : undefined
  const close = () => setOpenedFor(null)
  return (
    <Dropdown open={open} onToggle={() => setOpenedFor(open ? null : target)} align="left"
      trigger={<DropdownTrigger icon={workspace?.icon ?? 'fa-shield-halved'} label={workspace?.name ?? 'Permissions'} open={open} />}>
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
        <div class={styles.groupLabel}>Claude Code</div>
        <DropdownOption icon="fa-terminal" name="Change mode in Console"
          description={(claude ? `Last reported: ${claude}. ` : '') + 'At the idle prompt, Shift+Tab cycles the modes available to this session.'}
          selected={false} disabled={!tab || tab.status !== 'connected'}
          onClick={() => { close(); if (tab) bridge.openPermissionConsole(tab.id) }} />
      </>}
      <div class={styles.groupLabel}>Workspace tool approvals</div>
      {WORKSPACE_OPTIONS.map(opt =>
        <DropdownOption key={opt.value} icon={opt.icon} name={opt.name} description={opt.description}
          selected={workspace?.value === opt.value} disabled={!workspace}
          onClick={() => { close(); if (tab && workspace) bridge.changePermissionMode(tab.id, opt.value) }} />
      )}
    </Dropdown>
  )
}
