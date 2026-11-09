/** Browser counterpart of the GPUI shell. Tool/session state remains in BridgeHost. */
export type Panel = 'chat' | 'console' | 'agents'
export type Slot = 'main' | 'bottom' | 'right'
export interface ShellState {
  version: 1
  revision: number
  sidebarVisible: boolean
  sidebarWidth: number
  expandedProjects: string[]
  mainRatio: number
  rightWidth: number
  panels: Record<Slot, Panel[]>
  active: Record<Slot, Panel | null>
}
export type ShellOperation =
  | { type: 'sidebar'; visible: boolean }
  | { type: 'resize'; sidebarWidth?: number; mainRatio?: number; rightWidth?: number }
  | { type: 'project'; cwd: string; expanded: boolean }
  | { type: 'move'; panel: Panel; slot: Slot }
  | { type: 'activate'; panel: Panel }

export function defaultShellState(): ShellState {
  return { version: 1, revision: 0, sidebarVisible: true, sidebarWidth: 270,
    expandedProjects: [], mainRatio: .7, rightWidth: 280,
    panels: { main: ['chat'], bottom: ['console'], right: ['agents'] },
    active: { main: 'chat', bottom: 'console', right: 'agents' } }
}

const panels: Panel[] = ['chat', 'console', 'agents']
const slots: Slot[] = ['main', 'bottom', 'right']
function bounded(value: unknown, min: number, max: number): number {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < min || value > max) throw new Error('Invalid shell size')
  return value
}

/** Validate persisted data as well as incoming operations; never trust browser frames. */
export function validateShellState(input: unknown): ShellState {
  const s = input as ShellState
  if (!s || s.version !== 1 || !Number.isSafeInteger(s.revision) || s.revision < 0 ||
    typeof s.sidebarVisible !== 'boolean' || !Array.isArray(s.expandedProjects) || s.expandedProjects.length > 1000 ||
    s.expandedProjects.some(p => typeof p !== 'string' || p.length > 4096) || !s.panels || !s.active) throw new Error('Invalid shell state')
  bounded(s.sidebarWidth, 200, 600); bounded(s.mainRatio, .2, .85); bounded(s.rightWidth, 180, 700)
  const all: Panel[] = []
  for (const slot of slots) {
    const list = s.panels[slot]
    if (!Array.isArray(list) || list.some(p => !panels.includes(p)) ||
      (list.length ? !list.includes(s.active[slot] as Panel) : s.active[slot] !== null)) throw new Error('Invalid shell slot')
    all.push(...list)
  }
  if (all.length !== 3 || new Set(all).size !== 3) throw new Error('Every panel must have exactly one slot')
  return structuredClone(s)
}

export function applyShellOperation(state: ShellState, input: unknown): ShellState {
  const op = input as ShellOperation
  if (!op || typeof op !== 'object') throw new Error('Invalid shell operation')
  const next = structuredClone(state)
  switch (op.type) {
    case 'sidebar':
      if (typeof op.visible !== 'boolean') throw new Error('Invalid sidebar visibility')
      next.sidebarVisible = op.visible; break
    case 'resize':
      if (op.sidebarWidth !== undefined) next.sidebarWidth = bounded(op.sidebarWidth, 200, 600)
      if (op.mainRatio !== undefined) next.mainRatio = bounded(op.mainRatio, .2, .85)
      if (op.rightWidth !== undefined) next.rightWidth = bounded(op.rightWidth, 180, 700)
      break
    case 'project':
      if (typeof op.cwd !== 'string' || op.cwd.length > 4096 || typeof op.expanded !== 'boolean') throw new Error('Invalid project')
      next.expandedProjects = next.expandedProjects.filter(p => p !== op.cwd)
      if (op.expanded) next.expandedProjects.push(op.cwd)
      break
    case 'move':
      if (!panels.includes(op.panel) || !slots.includes(op.slot)) throw new Error('Invalid dock destination')
      for (const slot of slots) {
        next.panels[slot] = next.panels[slot].filter(p => p !== op.panel)
        if (next.active[slot] === op.panel) next.active[slot] = next.panels[slot][0] ?? null
      }
      next.panels[op.slot].push(op.panel); next.active[op.slot] = op.panel
      break
    case 'activate': {
      if (!panels.includes(op.panel)) throw new Error('Invalid panel')
      const slot = slots.find(slot => next.panels[slot].includes(op.panel))!
      next.active[slot] = op.panel; break
    }
    default: throw new Error('Unknown shell operation')
  }
  next.revision++
  return validateShellState(next)
}
