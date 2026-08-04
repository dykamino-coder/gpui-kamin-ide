import { useRef, useCallback } from 'preact/hooks'

/**
 * Input history navigation (like shell history with arrow up/down).
 * Initializes from JSONL user entries, appends new sends.
 */
export function useInputHistory() {
  const historyMap = useRef<Map<string, string[]>>(new Map())
  const indexMap = useRef<Map<string, number>>(new Map())
  const draftMap = useRef<Map<string, string>>(new Map())
  const seededTabs = useRef<Set<string>>(new Set())

  /** Seed history from JSONL entries (call when entries load/change) */
  const seedFromEntries = useCallback((tabId: string, entries: any[]) => {
    if (seededTabs.current.has(tabId)) return
    seededTabs.current.add(tabId)

    const history: string[] = []
    // Tracks whether the PREVIOUS non-empty user entry carried a
    // `<command-name>` marker. When a user invokes a slash command, CLI
    // writes THREE user entries in a row: the `<local-command-caveat>`
    // wrapper, the `<command-name>` block, then the expanded command body
    // (skill markdown, prompt text, etc.). The body itself has no marker
    // that distinguishes it from genuine user typing — so we infer it from
    // the preceding `<command-name>` entry.
    let prevWasCommandName = false
    for (const entry of entries) {
      if (entry.type !== 'user') continue
      const content = entry.message?.content
      let text = ''
      if (typeof content === 'string') {
        text = content
      } else if (Array.isArray(content)) {
        text = content
          .filter((b: any) => b.type === 'text')
          .map((b: any) => b.text || '')
          .join('\n')
      }
      // Skip CLI XML tags (system commands, not user input)
      if (!text.trim()) continue
      if (text.includes('<teammate-message')) continue
      if (text.includes('<local-command-caveat>') && !text.includes('<command-name>')) continue
      // Snapshot "previous entry was a <command-name> marker" at the top of
      // the iteration so the flag only survives ONE entry — the expanded
      // command body — and stale state doesn't leak if the body is
      // further followed by unrelated entries.
      const afterCommandName = prevWasCommandName
      prevWasCommandName = false
      if (afterCommandName) continue
      // Skip CLI-injected noise-wrapper user entries (they appear in JSONL as
      // `type:"user"` but are not real prompts the user typed — arrow-up
      // history should never surface their contents). List mirrors the tags
      // already stripped from chat rendering in utils.ts `hasCliXmlTags` /
      // `isCaveatOnlyEntry`.
      if (/<command-name>/i.test(text)) { prevWasCommandName = true; continue }
      if (/<(?:system-reminder|environment[_-]details|claude-mem-context|claude_background_info|fast_mode_info|env|task-notification|local-command-stdout|local-command-stderr|command-message|command-args|file-history-snapshot)[\s>]/i.test(text)) continue
      // Synthetic abort/cancel messages CLI injects into the JSONL stream
      // (SYNTHETIC_MESSAGES set in utils/messages.ts:302) — never a real
      // user prompt, don't surface in ↑ history.
      const trimmedRaw = text.trim()
      if (
        trimmedRaw === '[Request interrupted by user]' ||
        trimmedRaw === '[Request interrupted by user for tool use]' ||
        trimmedRaw === '[Skipped by user]' ||
        trimmedRaw.startsWith("The user doesn't want to take this action right now.") ||
        trimmedRaw === 'No response requested.' ||
        trimmedRaw.startsWith('[Request interrupted') ||
        trimmedRaw.startsWith('Unknown command:')
      ) continue
      // Skill/subcommand expansion payload that the sync system injects into
      // the JSONL stream — begins with "Base directory for this skill:" and
      // contains the full SKILL.md body, never a real user prompt.
      if (trimmedRaw.startsWith('Base directory for this skill:')) continue
      // Extract plain text from command entries
      const cmdMatch = text.match(/<command-name>([^<]+)<\/command-name>/)
      if (cmdMatch) {
        text = cmdMatch[1]
      }
      // Skip tool_result entries
      if (Array.isArray(content) && content.some((b: any) => b.type === 'tool_result')) continue

      const clean = text.replace(/<[^>]+>/g, '').trim()
      if (!clean) continue
      // Deduplicate consecutive
      if (history.length > 0 && history[history.length - 1] === clean) continue
      history.push(clean)
    }

    if (history.length > 0) {
      const existing = historyMap.current.get(tabId) || []
      // Merge: JSONL history first, then any in-memory sends
      historyMap.current.set(tabId, [...history, ...existing].slice(-100))
    }
  }, [])

  const push = useCallback((tabId: string, text: string) => {
    if (!text.trim()) return
    const history = historyMap.current.get(tabId) || []
    if (history.length > 0 && history[history.length - 1] === text) return
    history.push(text)
    if (history.length > 100) history.shift()
    historyMap.current.set(tabId, history)
    indexMap.current.delete(tabId)
    draftMap.current.delete(tabId)
  }, [])

  const navigate = useCallback((tabId: string, direction: 'up' | 'down', currentText: string): string | null => {
    const history = historyMap.current.get(tabId)
    if (!history || history.length === 0) return null

    let idx = indexMap.current.get(tabId)

    if (idx === undefined) {
      if (direction === 'down') return null
      draftMap.current.set(tabId, currentText)
      idx = history.length - 1
    } else if (direction === 'up') {
      if (idx <= 0) return null
      idx--
    } else {
      idx++
      if (idx >= history.length) {
        indexMap.current.delete(tabId)
        return draftMap.current.get(tabId) ?? ''
      }
    }

    indexMap.current.set(tabId, idx)
    return history[idx]
  }, [])

  const reset = useCallback((tabId: string) => {
    indexMap.current.delete(tabId)
    draftMap.current.delete(tabId)
  }, [])

  return { push, navigate, reset, seedFromEntries }
}
