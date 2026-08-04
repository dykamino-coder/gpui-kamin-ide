import { useState, useCallback } from 'preact/hooks'
import { SLASH_COMMANDS, type SlashCommand } from '../components/chat-header/slash-commands'
import { installedSkills } from '../signals/ui'

// Memoize the merged command list so we don't rebuild the array + Set on every
// keystroke. Keyed by the skills-signal reference — installedSkills is
// replaced whole-array when the list changes, so identity is enough.
const builtInSet = new Set(SLASH_COMMANDS.map(c => c.name))
let cachedSkillsRef: SlashCommand[] | null = null
let cachedMerged: SlashCommand[] = SLASH_COMMANDS.slice()
function getMergedCommands(skills: SlashCommand[]): SlashCommand[] {
  if (cachedSkillsRef === skills) return cachedMerged
  cachedSkillsRef = skills
  const dedup = skills.filter(s => !builtInSet.has(s.name))
  cachedMerged = dedup.length === 0 ? SLASH_COMMANDS.slice() : [...SLASH_COMMANDS, ...dedup]
  return cachedMerged
}

export interface SlashAutocompleteState {
  visible: boolean
  query: string
  matches: SlashCommand[]
  selectedIndex: number
}

export interface SlashAutocompleteHandlers {
  onInput: (value: string) => void
  onKeyDown: (e: KeyboardEvent) => boolean
  selectIndex: (index: number) => void
  applySelected: () => string | null
  hide: () => void
}

export function useSlashAutocomplete(): [SlashAutocompleteState, SlashAutocompleteHandlers] {
  const [state, setState] = useState<SlashAutocompleteState>({
    visible: false,
    query: '',
    matches: [],
    selectedIndex: -1,
  })

  const onInput = useCallback((value: string) => {
    if (!value.startsWith('/') || value.includes('\n')) {
      setState({ visible: false, query: '', matches: [], selectedIndex: -1 })
      return
    }

    const query = value.toLowerCase()
    // Merged list (built-ins + dedup'd skills) is memoized per skills snapshot
    // so we don't rebuild the 280+-item array on every keystroke.
    const all = getMergedCommands(installedSkills.value)
    // Match against full name OR the short skill slug (trailing segment after
    // the last `:` for plugin-namespaced commands). Lets users type
    // `/env-setup` and still find `/plugin:mcp-pipes@ai-hub:env-setup`.
    const matches = value === '/' ? all : all.filter(c => {
      const n = c.name.toLowerCase()
      if (n.startsWith(query)) return true
      const colonIdx = n.lastIndexOf(':')
      if (colonIdx > 0) {
        const shortSlug = '/' + n.slice(colonIdx + 1)
        if (shortSlug.startsWith(query)) return true
      }
      // Also match against the full name as substring (e.g. user types
      // `/test-generate` → matches `/plugin:unit-testing:test-generate`).
      if (n.includes(query.slice(1))) return true
      return false
    })

    if (matches.length === 0) {
      setState({ visible: false, query, matches: [], selectedIndex: -1 })
      return
    }

    setState({ visible: true, query, matches, selectedIndex: 0 })
  }, [])

  const selectIndex = useCallback((index: number) => {
    setState(s => ({ ...s, selectedIndex: index }))
  }, [])

  const applySelected = useCallback((): string | null => {
    const { matches, selectedIndex } = state
    if (selectedIndex < 0 || selectedIndex >= matches.length) return null
    const cmd = matches[selectedIndex].name
    return cmd === '/model' ? cmd + ' ' : cmd
  }, [state])

  const hide = useCallback(() => {
    setState({ visible: false, query: '', matches: [], selectedIndex: -1 })
  }, [])

  // Returns true if the key event was handled (should prevent default)
  const onKeyDown = useCallback((e: KeyboardEvent): boolean => {
    if (!state.visible) return false

    if (e.key === 'ArrowDown') {
      setState(s => ({
        ...s,
        selectedIndex: s.matches.length > 0 ? (s.selectedIndex + 1) % s.matches.length : -1,
      }))
      return true
    }
    if (e.key === 'ArrowUp') {
      setState(s => ({
        ...s,
        selectedIndex: s.matches.length > 0
          ? s.selectedIndex <= 0 ? s.matches.length - 1 : s.selectedIndex - 1
          : -1,
      }))
      return true
    }
    if (e.key === 'Tab') {
      return true
    }
    // Enter while the autocomplete list is open and a suggestion is highlighted
    // should COMPLETE the input, not send the half-typed message. The InputBar
    // key handler checks `handled` and runs `applySelected` when it sees
    // Enter/Tab here.
    if (e.key === 'Enter' && state.matches.length > 0 && state.selectedIndex >= 0) {
      return true
    }
    if (e.key === 'Escape') {
      hide()
      return true
    }
    return false
  }, [state.visible, state.matches.length, state.selectedIndex, hide])

  return [state, { onInput, onKeyDown, selectIndex, applySelected, hide }]
}
