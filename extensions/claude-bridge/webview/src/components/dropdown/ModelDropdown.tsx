import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { Dropdown } from './Dropdown'
import { DropdownTrigger } from './DropdownTrigger'
import { DropdownOption } from './DropdownOption'
import styles from './ModelDropdown.module.css'
import { currentModel, DEFAULT_MODEL_ID } from '../../signals/ui'
import { useBridge } from '../../hooks/useBridge'
import { activeTabId, tabs } from '../../signals/tabs'
import { codexCatalog, codexCatalogLoading, codexNotice, codexUnavailable, defaultEngine } from '../../signals/agent'
import {
  MODEL_OPTIONS,
  gptModelOptions,
  planModelSelection,
  selectedGptOption,
  selectedModelOption,
  type ModelEngine,
} from '../../lib/model-options'

const ENGINE_LABEL: Record<ModelEngine, string> = { claude: 'Claude', codex: 'ChatGPT' }

export function ModelDropdown(): JSX.Element {
  const [open, setOpen] = useState(false)
  const bridge = useBridge()
  const tabId = activeTabId.value
  const activeTab = tabId ? tabs.value.find(t => t.id === tabId) : undefined
  const tabEngine: ModelEngine | null = activeTab ? (activeTab.engine ?? 'claude') : null
  const gpt = gptModelOptions(codexCatalog.value?.models ?? [])
  const notice = codexNotice(codexCatalog.value)
  // No working codex on the server: the group stays visible but inert.
  const gptOff = codexUnavailable(codexCatalog.value)

  // Per-tab model wins — the session's own history (JSONL → lastModel →
  // tab.model) is authoritative. For fresh sessions with no prior model, the
  // default is DEFAULT_MODEL_ID regardless of what the user last picked in a
  // different tab. `currentModel.value` is only consulted when there is no
  // active tab at all (e.g. landing screen). A GPT chat shows its GPT model.
  const model = activeTab ? (activeTab.model || (tabEngine === 'codex' ? '' : DEFAULT_MODEL_ID)) : currentModel.value
  const current = tabEngine === 'codex' ? selectedGptOption(model, gpt) : selectedModelOption(model)
  const listed = tabEngine === 'codex' ? gpt : MODEL_OPTIONS
  const isOutsidePicker = !listed.some(option => option.value === current.value)

  function toggle(): void {
    const next = !open
    setOpen(next)
    // Fresh GPT list + account state every time the menu opens.
    if (next && tabId) {
      codexCatalogLoading.value = true
      bridge.requestCodexModels(tabId)
    }
  }

  function handleSelect(value: string): void {
    setOpen(false)
    const plan = planModelSelection(tabEngine, value)
    if (plan.kind === 'remember') {
      currentModel.value = value
      return
    }
    if (plan.kind === 'new-chat') {
      bridge.newAgentSession(plan.engine, plan.model)
      return
    }
    currentModel.value = value
    if (tabId) bridge.changeModel(tabId, value)
  }

  function handleMainAgent(engine: ModelEngine): void {
    defaultEngine.value = engine
    bridge.setDefaultEngine(tabId ?? '', engine)
  }

  const isSelected = (value: string): boolean => (model || current.value) === value

  return (
    <Dropdown
      open={open}
      onToggle={toggle}
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
      <div class={styles.groupLabel}>Claude</div>
      {MODEL_OPTIONS.map(opt => (
        <DropdownOption
          key={opt.value}
          icon={opt.icon}
          name={opt.name}
          description={tabEngine === 'codex' ? 'Opens a new Claude chat' : opt.description}
          selected={tabEngine !== 'codex' && isSelected(opt.value)}
          onClick={() => handleSelect(opt.value)}
        />
      ))}
      <div class={styles.groupLabel}>ChatGPT</div>
      {notice && <div class={styles.notice} role="status">{notice}</div>}
      {gpt.length === 0 && !notice && (
        <div class={styles.notice}>{codexCatalogLoading.value ? 'Loading GPT models…' : 'No GPT models reported by the server.'}</div>
      )}
      {gpt.map(opt => (
        <DropdownOption
          key={opt.value}
          icon={opt.icon}
          name={opt.name}
          description={tabEngine === 'codex' ? opt.description : 'Opens a new ChatGPT chat'}
          selected={tabEngine === 'codex' && isSelected(opt.value)}
          disabled={gptOff}
          onClick={() => handleSelect(opt.value)}
        />
      ))}
      <div class={styles.groupLabel}>Main agent for new chats</div>
      {(['claude', 'codex'] as const).map(engine => (
        <DropdownOption
          key={engine}
          icon={engine === 'claude' ? 'fa-gem' : 'fa-robot'}
          name={ENGINE_LABEL[engine]}
          description={engine === 'claude' ? 'New chats run Claude Code' : gptOff ? 'Unavailable on this server' : 'New chats run ChatGPT through Codex'}
          selected={defaultEngine.value === engine}
          disabled={engine === 'codex' && gptOff && defaultEngine.value !== 'codex'}
          onClick={() => handleMainAgent(engine)}
        />
      ))}
    </Dropdown>
  )
}
