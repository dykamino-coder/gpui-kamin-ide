import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { Dropdown } from './Dropdown'
import { DropdownTrigger } from './DropdownTrigger'
import { DropdownOption } from './DropdownOption'
import { currentPermission } from '../../signals/ui'
import { useBridge } from '../../hooks/useBridge'
import { activeTabId } from '../../signals/tabs'
import { jsonlEntriesByTab } from '../../signals/jsonl'

const PERM_OPTIONS = [
  { value: 'default', icon: 'fa-shield-halved', name: 'Default', description: 'Ask before every action' },
  { value: 'acceptEdits', icon: 'fa-code', name: 'Accept edits', description: 'Auto-accept file edits, ask for shell' },
  { value: 'plan', icon: 'fa-clipboard-list', name: 'Plan mode', description: 'Plan first, then ask to execute' },
  { value: 'auto', icon: 'fa-wand-magic-sparkles', name: 'Auto', description: 'AI decides what is safe to run' },
  { value: 'dontAsk', icon: 'fa-forward', name: "Don't ask", description: 'Accept everything without prompts' },
  { value: 'bypassPermissions', icon: 'fa-triangle-exclamation', name: 'Bypass permissions', description: 'Skip all permission checks' },
] as const

function inferTabPermissionMode(tabId: string | null): string | null {
  if (!tabId) return null
  const entries = jsonlEntriesByTab.value.get(tabId)
  if (!entries) return null
  // Walk backwards for the freshest permissionMode — either the dedicated
  // `permission-mode`/`mode` entry or the inline field on user/assistant ones.
  for (let i = entries.length - 1; i >= 0; i--) {
    const e = entries[i]
    if (!e) continue
    if ((e.type === 'mode' || (e.type as string) === 'permission-mode') && typeof (e as any).mode === 'string') {
      return (e as any).mode
    }
    if ((e as any).permissionMode) return (e as any).permissionMode
  }
  return null
}

export function PermissionsDropdown(): JSX.Element {
  const [open, setOpen] = useState(false)
  const bridge = useBridge()
  const tabId = activeTabId.value
  const tabPermission = inferTabPermissionMode(tabId)
  const permission = tabPermission ?? currentPermission.value

  const current = PERM_OPTIONS.find(o => o.value === permission) ?? PERM_OPTIONS[3]

  function handleSelect(value: string): void {
    currentPermission.value = value
    setOpen(false)
    if (!tabId) return
    bridge.changePermissionMode(tabId, value)
  }

  return (
    <Dropdown
      open={open}
      onToggle={() => setOpen(v => !v)}
      align="left"
      trigger={
        <DropdownTrigger
          icon={current.icon}
          label={current.name}
          open={open}
        />
      }
    >
      {PERM_OPTIONS.map(opt => (
        <DropdownOption
          key={opt.value}
          icon={opt.icon}
          name={opt.name}
          description={opt.description}
          selected={permission === opt.value}
          onClick={() => handleSelect(opt.value)}
        />
      ))}
    </Dropdown>
  )
}
