import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { Dropdown } from './Dropdown'
import { DropdownTrigger } from './DropdownTrigger'
import { DropdownOption } from './DropdownOption'
import { currentModel, DEFAULT_MODEL_ID } from '../../signals/ui'
import { useBridge } from '../../hooks/useBridge'
import { activeTabId, tabs } from '../../signals/tabs'

const MODEL_OPTIONS = [
  // Opus 5: 1M контекст НАТИВНО (как Fable 5) — отдельная [1m]-запись не
  // нужна. 4.8 выпилен целиком (как раньше 4.7); cliName явный, не alias
  // 'opus' — alias у старого CLI мог резолвиться в 4.8.
  { value: 'claude-opus-5', cliName: 'claude-opus-5', icon: 'fa-gem', name: 'Opus 5', description: 'Most capable, 1M-token context' },
  { value: 'claude-sonnet-5', cliName: 'claude-sonnet-5', icon: 'fa-feather', name: 'Sonnet 5', description: 'Fast and balanced' },
  { value: 'claude-haiku-4-5', cliName: 'haiku', icon: 'fa-bolt', name: 'Haiku 4.5', description: 'Fastest, lightweight tasks' },
  { value: 'claude-fable-5', cliName: 'claude-fable-5', icon: 'fa-book-open', name: 'Fable 5', description: 'Expressive, creative writing' },
] as const

export function ModelDropdown(): JSX.Element {
  const [open, setOpen] = useState(false)
  const bridge = useBridge()
  const tabId = activeTabId.value
  const activeTab = tabId ? tabs.value.find(t => t.id === tabId) : undefined

  // Per-tab model wins — the session's own history (JSONL → lastModel →
  // tab.model) is authoritative. For fresh sessions with no prior model, the
  // default is DEFAULT_MODEL_ID regardless of what the user last picked in a
  // different tab. `currentModel.value` is only consulted when there is no
  // active tab at all (e.g. landing screen).
  const tabModelValue = activeTab?.model
    ? (MODEL_OPTIONS.find(o => o.cliName === activeTab.model || o.value === activeTab.model)?.value ?? null)
    : null
  const model = tabModelValue ?? (activeTab ? DEFAULT_MODEL_ID : currentModel.value)

  const current = MODEL_OPTIONS.find(o => o.value === model) ?? MODEL_OPTIONS[0]

  function handleSelect(value: string): void {
    currentModel.value = value
    setOpen(false)
    if (!tabId) return
    const opt = MODEL_OPTIONS.find(o => o.value === value)
    bridge.changeModel(tabId, opt?.cliName ?? value)
  }

  return (
    <Dropdown
      open={open}
      onToggle={() => setOpen(v => !v)}
      align="right"
      trigger={
        <DropdownTrigger
          icon={current.icon}
          label={current.name}
          open={open}
        />
      }
    >
      {MODEL_OPTIONS.map(opt => (
        <DropdownOption
          key={opt.value}
          icon={opt.icon}
          name={opt.name}
          description={opt.description}
          selected={model === opt.value}
          onClick={() => handleSelect(opt.value)}
        />
      ))}
    </Dropdown>
  )
}
