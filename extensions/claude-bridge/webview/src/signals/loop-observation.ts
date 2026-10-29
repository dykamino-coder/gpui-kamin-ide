import { computed } from '@preact/signals'
import { tabs } from './tabs'
import { jsonlEntriesByTab, getJsonlStructureVersion } from './jsonl'
import { analyzeTurns, ProjectTurnHistory, type TurnAnalysis } from '../utils/loop-observation'

// Webview-local numeric history only; discarded on reload. No transcript
// mutation, persistence, network request, or agent-facing side effect.
const history = new ProjectTurnHistory()
const cache = new Map<string, { key: string; analysis: TurnAnalysis }>()
let previousKey = ''
let previous: Map<string, { current: TurnAnalysis['current']; baseline: ReturnType<ProjectTurnHistory['observe']> }> =
  new Map()

export const loopObservations = computed(() => {
  const openTabs = tabs.value
  const entries = jsonlEntriesByTab.value
  const stateKey = JSON.stringify(
    openTabs.map((t) => [
      t.id,
      t.sessionId,
      t.conversationId,
      t.connectionAuthority,
      t.cwd,
      getJsonlStructureVersion(t.id),
    ]),
  )
  if (stateKey === previousKey) return previous
  const analyses = new Map<string, TurnAnalysis>()
  const projects = new Map<string, string>()
  const liveIds = new Set(openTabs.map((t) => t.id))
  for (const id of cache.keys()) if (!liveIds.has(id)) cache.delete(id)
  for (const tab of openTabs) {
    const project = JSON.stringify([tab.connectionAuthority, tab.cwd])
    projects.set(tab.id, project)
    const key = JSON.stringify([tab.sessionId, tab.conversationId, getJsonlStructureVersion(tab.id)])
    let cached = cache.get(tab.id)
    if (cached?.key !== key) {
      cached = { key, analysis: analyzeTurns(entries.get(tab.id) ?? []) }
      cache.set(tab.id, cached)
    }
    analyses.set(tab.id, cached.analysis)
    history.observe(project, tab.conversationId ?? tab.sessionId ?? tab.id, cached.analysis.completed)
  }
  previousKey = stateKey
  previous = new Map(
    openTabs.map((tab) => [
      tab.id,
      {
        current: analyses.get(tab.id)!.current,
        baseline: history.observe(projects.get(tab.id)!, '', []),
      },
    ]),
  )
  return previous
})
