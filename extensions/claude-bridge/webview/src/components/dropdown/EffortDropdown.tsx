import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { Dropdown } from './Dropdown'
import { DropdownTrigger } from './DropdownTrigger'
import { DropdownOption } from './DropdownOption'
import { currentEffort } from '../../signals/ui'
import { useBridge } from '../../hooks/useBridge'
import { activeTabId, tabs } from '../../signals/tabs'

// The CLI's `--effort` scale, verbatim (`claude --help`: low, medium, high,
// xhigh, max). Values flow to `--effort <value>` + CLAUDE_CODE_EFFORT_LEVEL, so
// they MUST match the CLI exactly — an unknown value aborts session start.
// NB: `ultracode` is NOT an effort level (it's xhigh + workflows, toggled
// separately), so it is deliberately absent here.
const EFFORT_OPTIONS = [
  { value: 'low', icon: 'fa-bolt', name: 'Low', description: 'Quick responses, less reasoning' },
  { value: 'medium', icon: 'fa-gauge', name: 'Medium', description: 'Moderate thinking depth' },
  { value: 'high', icon: 'fa-lightbulb', name: 'High', description: 'Deep reasoning, thorough analysis' },
  { value: 'xhigh', icon: 'fa-fire', name: 'Extra high', description: 'Extended reasoning for hard problems' },
  { value: 'max', icon: 'fa-brain', name: 'Max', description: 'Maximum reasoning depth' },
] as const

/** Shown when a ChatGPT chat runs on a level Bridge does not know: the
 *  app-server reported none and the catalog names no default for the model, so
 *  turns go without effort and the engine picks its own. */
const ENGINE_DEFAULT = { value: 'default', icon: 'fa-circle-half-stroke', name: 'Default', description: "The engine's own level" } as const

type EffortOption = (typeof EFFORT_OPTIONS)[number] | typeof ENGINE_DEFAULT

/** The level the selector shows for a tab. A Claude chat without a level runs
 *  on `--effort high` (server session-env.ts), so High is its truth. A ChatGPT
 *  chat reports the level its turns are sent with (server session-codex.ts);
 *  until it does, or when it has none, the selector says Default instead of
 *  naming a level the engine does not run with (REAL-121). */
export function effortShown(
  tab: { engine?: string | null; effort?: string | null } | undefined,
  lastPicked: string | null | undefined,
): EffortOption {
  const codex = tab?.engine === 'codex'
  const effort = codex ? tab?.effort : (tab?.effort ?? lastPicked)
  return EFFORT_OPTIONS.find(o => o.value === effort)
    ?? (codex ? ENGINE_DEFAULT : EFFORT_OPTIONS.find(o => o.value === 'high') ?? EFFORT_OPTIONS[2])
}

export function EffortDropdown(): JSX.Element {
  const [open, setOpen] = useState(false)
  const bridge = useBridge()
  const tabId = activeTabId.value
  const activeTab = tabId ? tabs.value.find(t => t.id === tabId) : undefined
  const current = effortShown(activeTab, currentEffort.value)
  const effort = current.value

  function handleSelect(value: string): void {
    currentEffort.value = value
    setOpen(false)
    if (!tabId) return
    if (value === 'default') return
    bridge.changeEffort(tabId, value)
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
      {EFFORT_OPTIONS.map(opt => (
        <DropdownOption
          key={opt.value}
          icon={opt.icon}
          name={opt.name}
          description={opt.description}
          selected={effort === opt.value}
          onClick={() => handleSelect(opt.value)}
        />
      ))}
    </Dropdown>
  )
}
