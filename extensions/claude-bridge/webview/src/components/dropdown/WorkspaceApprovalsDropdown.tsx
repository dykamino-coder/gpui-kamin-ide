import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import styles from './ModelDropdown.module.css'
import { Dropdown } from './Dropdown'
import { DropdownTrigger } from './DropdownTrigger'
import { DropdownOption } from './DropdownOption'
import { useBridge } from '../../hooks/useBridge'
import { activeTabId, tabs } from '../../signals/tabs'
import type { WorkspacePermissionMode } from '../../../shared/types'

const WORKSPACE_OPTIONS = [
  { value: 'ask', icon: 'fa-shield-halved', name: 'Use approval rules', description: 'Ask for tools that need approval.' },
  { value: 'readOnly', icon: 'fa-book-open', name: 'Read tools only', description: 'Allow file reads and searches. Block edits, shell and external tools.' },
  { value: 'acceptEdits', icon: 'fa-code', name: 'Accept file edits', description: 'Allow file edits by default. Explicit tool rules still apply.' },
  { value: 'dontAsk', icon: 'fa-ban', name: 'Deny on prompt', description: 'Deny any tool that would require a prompt.' },
  { value: 'bypassPermissions', icon: 'fa-triangle-exclamation', name: 'Allow workspace tools', description: 'Skip the workspace host approval prompt.' },
] satisfies { value: WorkspacePermissionMode; icon: string; name: string; description: string }[]

export function WorkspaceApprovalsDropdown(): JSX.Element {
  const [openedFor, setOpenedFor] = useState<string | null>(null)
  const bridge = useBridge()
  const tab = tabs.value.find(t => t.id === activeTabId.value)
  const target = tab ? JSON.stringify([tab.id, tab.engine, tab.sessionId]) : null
  const open = target !== null && openedFor === target
  const workspace = WORKSPACE_OPTIONS.find(o => o.value === tab?.workspacePermission)
  const close = () => setOpenedFor(null)
  return (
    <Dropdown open={open} onToggle={() => setOpenedFor(open ? null : target)} align="left"
      trigger={<DropdownTrigger icon={workspace?.icon ?? 'fa-shield-halved'} label="Bridge tools" open={open} />}>
      <div class={styles.groupLabel}>Workspace tool approvals</div>
      {WORKSPACE_OPTIONS.map(opt =>
        <DropdownOption key={opt.value} icon={opt.icon} name={opt.name} description={opt.description}
          selected={workspace?.value === opt.value} disabled={!workspace}
          onClick={() => { close(); if (tab && workspace) bridge.changePermissionMode(tab.id, opt.value) }} />
      )}
    </Dropdown>
  )
}
