import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { Dropdown } from './Dropdown'
import { DropdownTrigger } from './DropdownTrigger'
import { DropdownOption } from './DropdownOption'
import { currentModel, DEFAULT_MODEL_ID } from '../../signals/ui'
import { useBridge } from '../../hooks/useBridge'
import { activeTabId, tabs } from '../../signals/tabs'
import { MODEL_OPTIONS, selectedModelOption } from '../../lib/model-options'

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
  const model = activeTab ? (activeTab.model || DEFAULT_MODEL_ID) : currentModel.value
  const current = selectedModelOption(model)
  const isOutsidePicker = !MODEL_OPTIONS.some(option => option.value === current.value)

  function handleSelect(value: string): void {
    currentModel.value = value
    setOpen(false)
    if (!tabId) return
    bridge.changeModel(tabId, value)
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
      {isOutsidePicker && (
        <DropdownOption
          icon={current.icon}
          name={current.name}
          description={current.description}
          selected
          onClick={() => setOpen(false)}
        />
      )}
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
