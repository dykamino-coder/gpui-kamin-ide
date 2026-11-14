import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { Dropdown } from './Dropdown'
import { DropdownTrigger } from './DropdownTrigger'
import { DropdownOption } from './DropdownOption'
import styles from './ModelDropdown.module.css'
import { currentModel, DEFAULT_MODEL_ID } from '../../signals/ui'
import { useBridge } from '../../hooks/useBridge'
import { activeTabId, tabs } from '../../signals/tabs'
import { codexCatalog, codexCatalogLoading, dashboardUrl, defaultEngine, engineAuth, engineBlock, NO_ENGINE_NOTICE } from '../../signals/agent'
import {
  MODEL_OPTIONS,
  engineOfModel,
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
  // An engine the server cannot run (signed out, or no codex) stays visible but
  // inert, with the reason; null = it runs (or the server has not said yet).
  const blocked: Record<ModelEngine, string | null> = {
    claude: engineBlock('claude', engineAuth.value, codexCatalog.value),
    codex: engineBlock('codex', engineAuth.value, codexCatalog.value),
  }
  const noEngine = blocked.claude !== null && blocked.codex !== null
  const notice = blocked.codex ?? codexCatalog.value?.error?.message ?? null

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
    if (blocked[engineOfModel(value)]) return
    const plan = planModelSelection(tabEngine, value)
    if (plan.kind === 'remember') {
      currentModel.value = value
      return
    }
    if (plan.kind === 'new-chat') {
      bridge.newAgentSession(plan.engine, plan.model)
      return
    }
    // 'switch': the same chat continues on the other agent (the connection
    // routes a model of the other engine to an engine switch).
    if (plan.kind === 'change') currentModel.value = value
    if (tabId) bridge.changeModel(tabId, value)
  }

  function openDashboard(): void {
    setOpen(false)
    void bridge.getConfig()
      .then(cfg => { if (cfg?.serverUrl) return bridge.openExternal(dashboardUrl(cfg.serverUrl)) })
      .catch(() => { /* no server configured: nothing to open */ })
  }

  function mainAgentDescription(engine: ModelEngine): string {
    const reason = blocked[engine]
    if (!reason) return engine === 'claude' ? 'New chats run Claude Code' : 'New chats run ChatGPT through Codex'
    const other: ModelEngine = engine === 'claude' ? 'codex' : 'claude'
    // Already the main agent: new chats open on the signed-in engine instead.
    if (defaultEngine.value === engine && !blocked[other]) return `${reason} New chats open on ${ENGINE_LABEL[other]}.`
    return reason
  }

  function handleMainAgent(engine: ModelEngine): void {
    if (blocked[engine]) return
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
      {noEngine && (
        <div class={styles.notice} role="status">
          {NO_ENGINE_NOTICE}{' '}
          <button type="button" class={styles.noticeLink} onClick={openDashboard}>Open the Bridge dashboard</button>
        </div>
      )}
      <div class={styles.groupLabel}>Claude</div>
      {blocked.claude && !noEngine && <div class={styles.notice} role="status">{blocked.claude}</div>}
      {MODEL_OPTIONS.map(opt => (
        <DropdownOption
          key={opt.value}
          icon={opt.icon}
          name={opt.name}
          description={tabEngine === 'codex' ? 'Continues this chat on Claude' : opt.description}
          selected={tabEngine !== 'codex' && isSelected(opt.value)}
          disabled={blocked.claude !== null}
          onClick={() => handleSelect(opt.value)}
        />
      ))}
      <div class={styles.groupLabel}>ChatGPT</div>
      {notice && !noEngine && <div class={styles.notice} role="status">{notice}</div>}
      {gpt.length === 0 && !notice && (
        <div class={styles.notice}>{codexCatalogLoading.value ? 'Loading GPT models…' : 'No GPT models reported by the server.'}</div>
      )}
      {gpt.map(opt => (
        <DropdownOption
          key={opt.value}
          icon={opt.icon}
          name={opt.name}
          description={tabEngine === 'claude' ? 'Continues this chat on ChatGPT' : tabEngine === null ? 'Opens a new ChatGPT chat' : opt.description}
          selected={tabEngine === 'codex' && isSelected(opt.value)}
          disabled={blocked.codex !== null}
          onClick={() => handleSelect(opt.value)}
        />
      ))}
      <div class={styles.groupLabel}>Main agent for new chats</div>
      {(['claude', 'codex'] as const).map(engine => (
        <DropdownOption
          key={engine}
          icon={engine === 'claude' ? 'fa-gem' : 'fa-robot'}
          name={ENGINE_LABEL[engine]}
          description={mainAgentDescription(engine)}
          selected={defaultEngine.value === engine}
          disabled={blocked[engine] !== null && defaultEngine.value !== engine}
          onClick={() => handleMainAgent(engine)}
        />
      ))}
    </Dropdown>
  )
}
