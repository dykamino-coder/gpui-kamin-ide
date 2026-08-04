import type { TreeNode } from '../../shared/types'
import type { JsonlEntryData } from '../types/jsonl'
import { tabAgentTrees, tabJsonlLive, recordAgentHistory, findAgentByName, type AgentInfo, type AgentTreeState } from '../signals/agents'
import { sessionTree } from '../signals/tabs'

function getOrCreateTree(tabId: string): AgentTreeState {
  const map = tabAgentTrees.value
  let tree = map.get(tabId)
  if (!tree) {
    tree = {
      teams: new Map(),
      standaloneAgents: new Map(),
      pendingAgentCalls: new Map(),
      teamNameAliases: new Map(),
      taskIdToAgent: new Map(),
      agentIdToAgent: new Map(),
    }
    // Mutate the inner map — signal will be triggered by callers
    map.set(tabId, tree)
  }
  return tree
}

/** Parse JSONL entries for agent/team activity. Returns true if anything changed.
 *  РЕПЛЕЙ ТОЖЕ строит дерево (инцидент «агенты работают, а Active 0»): пары
 *  tool_use→tool_result и idle-маркеры закрываются к концу реплея сами, так что
 *  давно умершие агенты приходят к done и уходят прунером в history, а агенты
 *  с НЕзакрытой парой — живые (PTY переживает рестарт клиента) — остаются
 *  running. Панель и так рендерит active только после перехода в live, поэтому
 *  промежуточные running реплея не мелькают. Страховка от ложного running при
 *  мёртвом PTY — staleness-прунер (STALE_RUNNING_MS). */
export function parseAgentEntries(tabId: string, entries: any[]): boolean {
  const tree = getOrCreateTree(tabId)
  let changed = false

  for (const entry of entries) {
    if (entry.type === 'assistant') {
      const content = entry.message?.content
      if (!Array.isArray(content)) continue

      for (const block of content) {
        if (block.type !== 'tool_use') continue

        if (block.name === 'TeamCreate') {
          const teamName = block.input?.team_name
          if (teamName) {
            const existing = tree.teams.get(teamName)
            if (!existing) {
              tree.teams.set(teamName, { name: teamName, description: block.input?.description || '', status: 'active', agents: new Map() })
              changed = true
            } else {
              if (!existing.description && block.input?.description) { existing.description = block.input.description; changed = true }
              if (existing.status !== 'active') { existing.status = 'active'; changed = true } // re-created after a disband
            }
            // Re-parent any standalone agents that referenced this team but
            // were parked as solo because their Agent tool_use was parsed
            // before the TeamCreate had been seen.
            const target = tree.teams.get(teamName)!
            for (const [name, agent] of [...tree.standaloneAgents]) {
              if (agent.teamName === teamName) {
                target.agents.set(name, agent)
                tree.standaloneAgents.delete(name)
                changed = true
              }
            }
          }
        }

        if (block.name === 'Agent') {
          const agentName = block.input?.name || block.input?.description || 'agent'
          const rawTeamName = block.input?.team_name
          let teamName: string | undefined = rawTeamName

          if (teamName) {
            if (tree.teamNameAliases.has(teamName)) teamName = tree.teamNameAliases.get(teamName)!
            // Auto-create the team if Agent names one that we haven't seen yet —
            // the Agent tool_use can legally arrive before TeamCreate in the same
            // tool burst, and we must not scatter its members into standaloneAgents.
            if (!tree.teams.has(teamName)) {
              tree.teams.set(teamName, { name: teamName, description: '', agents: new Map() })
            }
          }

          const agent: AgentInfo = {
            id: block.id,
            name: agentName,
            inputName: agentName,
            description: block.input?.description || '',
            teamName,
            status: 'running',
            agentType: block.input?.subagent_type,
            messages: [],
            lastSeenAt: entry.timestamp,
          }
          tree.pendingAgentCalls.set(block.id, agent)
          if (teamName && tree.teams.has(teamName)) {
            tree.teams.get(teamName)!.agents.set(agentName, agent)
            tree.standaloneAgents.delete(agentName)
          } else {
            tree.standaloneAgents.set(agentName, agent)
          }
          changed = true
        }

        if (block.name === 'TeamDelete') {
          const teamName = block.input?.team_name
          if (teamName) {
            // Disband, don't DROP: mark the team + its still-running members so the
            // Agents panel shows the team was dissolved (was silently deleted → the
            // user couldn't tell a team from a finished one). A re-created team
            // flips back to active above.
            const disband = (name: string): void => {
              const t = tree.teams.get(name)
              if (!t || t.status === 'disbanded') return
              t.status = 'disbanded'
              for (const a of t.agents.values()) if (a.status === 'running') a.status = 'terminated'
              changed = true
            }
            disband(teamName)
            const alias = tree.teamNameAliases.get(teamName)
            if (alias) disband(alias)
            for (const [, friendlyName] of tree.teamNameAliases) if (friendlyName === teamName) disband(friendlyName)
          }
        }

        if (block.name === 'SendMessage') {
          const recipient = block.input?.recipient
          const msgContent = block.input?.content
          const msgType = block.input?.type
          if (recipient && msgContent) {
            for (const team of tree.teams.values()) {
              const agent = team.agents.get(recipient)
              if (agent) { agent.messages.push({ from: 'team-lead', text: String(msgContent).substring(0, 500), ts: Date.now() }); changed = true }
            }
            const contentStr = String(msgContent)
            const isShutdown = msgType === 'shutdown_request' || contentStr.includes('shutdown_request')
            if (isShutdown) {
              for (const team of tree.teams.values()) {
                const agent = team.agents.get(recipient)
                if (agent) { agent.status = 'terminated'; changed = true } // lead kicked it — not a natural finish
              }
            }
          }
        }
      }
    }

    if (entry.type === 'user') {
      const content = entry.message?.content
      if (!Array.isArray(content)) continue
      for (const block of content) {
        if (block.type === 'tool_result' && block.tool_use_id) {
          const pending = tree.pendingAgentCalls.get(block.tool_use_id)
          if (pending) {
            pending.lastSeenAt = entry.timestamp || pending.lastSeenAt
            // Structured result (sibling of `message` on the entry) — richer and
            // more reliable than scraping the text below. Additive: it fills the
            // completion stats + a definitive status; the regex still handles the
            // name/agent_id the text carries. Typed via JsonlEntryData so a field
            // rename is caught by the compiler, not silently dropped.
            const resultText = typeof block.content === 'string' ? block.content
              : Array.isArray(block.content) ? block.content.map((b: any) => b.text || '').join('\n') : ''
            // BACKGROUND-спавн: «Async agent launched … agentId: X» — эхо запуска,
            // не финал. Детект ДО структурной ветки: CLI кладёт в toolUseResult
            // спавн-эха status 'completed', и агент гас в done мгновенно
            // (прод: работающие be2/fe2 в Completed при Active 0; воспроизведено
            // стендом LongWorker).
            const asyncLaunch = /Async agent launched/i.test(resultText)
            const tur = (entry as JsonlEntryData).toolUseResult
            if (tur) {
              if (typeof tur.agentType === 'string' && !pending.agentType) pending.agentType = tur.agentType
              if (typeof tur.totalTokens === 'number') pending.totalTokens = tur.totalTokens
              if (typeof tur.totalToolUseCount === 'number') pending.totalToolUseCount = tur.totalToolUseCount
              if (typeof tur.totalDurationMs === 'number') pending.durationMs = tur.totalDurationMs
              if (!asyncLaunch) {
                if (tur.status === 'completed' && pending.status === 'running') { pending.status = 'done'; changed = true }
                else if ((tur.status === 'error' || tur.status === 'failed') && pending.status === 'running') { pending.status = 'error'; changed = true }
              }
            }
            const nameMatch = resultText.match(/\bname:\s*(\S+)/)
            const agentIdMatch = resultText.match(/\bagent_id:\s*(\S+)/)

            if (nameMatch && nameMatch[1] !== pending.name) {
              const oldName = pending.name
              const newName = nameMatch[1]
              pending.name = newName
              if (pending.teamName && tree.teams.has(pending.teamName)) {
                const team = tree.teams.get(pending.teamName)!
                team.agents.delete(oldName)
                team.agents.set(newName, pending)
              } else {
                tree.standaloneAgents.delete(oldName)
                tree.standaloneAgents.set(newName, pending)
              }
              changed = true
            }
            if (agentIdMatch) {
              pending.agentId = agentIdMatch[1]
              tree.agentIdToAgent.set(agentIdMatch[1], pending)
            }
            // BACKGROUND-агент: agentId из спавн-эха вяжем для
            // task-notification-финала ниже (asyncLaunch объявлен выше).
            if (asyncLaunch) {
              const bgId = resultText.match(/\bagentId:\s*([A-Za-z0-9_-]+)/)
              if (bgId) { pending.agentId = bgId[1]; tree.agentIdToAgent.set(bgId[1], pending) }
            }
            if (pending.status === 'running') {
              if (block.is_error) { pending.status = 'error'; changed = true }
              // `teammate_spawned` = the Agent tool RETURNED (the teammate is now
              // running ASYNC) — NOT completion. Marking done here flipped a
              // just-spawned teammate to DONE while it was still generating (its
              // status showed "done" the instant the panel opened). A blocking
              // subagent's tool_result carries no such status, so it still
              // completes here; a teammate finishes via its idle_notification below.
              else if (!pending.teamName && !asyncLaunch && tur?.status !== 'teammate_spawned') { pending.status = 'done'; changed = true }
            }
          }
        }
      }
    }

    if (entry.type === 'user') {
      const content = entry.message?.content
      const textParts: string[] = []
      if (typeof content === 'string') {
        textParts.push(content)
      } else if (Array.isArray(content)) {
        for (const block of content) {
          if (block.type === 'text' && block.text) textParts.push(block.text)
          if (block.type === 'tool_result' && block.content) {
            if (typeof block.content === 'string') textParts.push(block.content)
            else if (Array.isArray(block.content)) {
              for (const sub of block.content) {
                if (sub.type === 'text' && sub.text) textParts.push(sub.text)
              }
            }
          }
        }
      }
      const text = textParts.join('\n')
      // Финал background-агента: task-notification с его agentId (= task-id) и
      // терминальным статусом. Это ЕДИНСТВЕННЫЙ сигнал завершения асинхронного
      // Agent-вызова — tool_result у него был мгновенным спавн-эхом.
      if (text.includes('<task-notification>')) {
        const ntfRegex = /<task-notification>[\s\S]*?<task-id>([A-Za-z0-9_-]+)<\/task-id>[\s\S]*?<status>(\w+)<\/status>[\s\S]*?<\/task-notification>/g
        let ntf: RegExpExecArray | null
        while ((ntf = ntfRegex.exec(text)) !== null) {
          const agent = tree.agentIdToAgent.get(ntf[1])
          if (agent && agent.status === 'running') {
            agent.status = ntf[2] === 'completed' ? 'done' : 'error'
            agent.lastSeenAt = entry.timestamp || agent.lastSeenAt
            changed = true
          }
        }
      }
      const tmRegex = /<teammate-message\s+teammate_id="([^"]+)"[^>]*>([\s\S]*?)<\/teammate-message>/g
      let tmMatch: RegExpExecArray | null
      while ((tmMatch = tmRegex.exec(text)) !== null) {
        const senderId = tmMatch[1]
        const msgBody = tmMatch[2].trim()
        let msgText = msgBody
        let msgType = ''
        try {
          const parsed = JSON.parse(msgBody)
          if (parsed && typeof parsed === 'object') { msgType = parsed.type || ''; msgText = parsed.summary || parsed.message || parsed.type || msgBody }
        } catch { /* not json */ }

        let foundAgent: AgentInfo | undefined
        for (const team of tree.teams.values()) {
          foundAgent = team.agents.get(senderId)
          if (foundAgent) break
        }
        if (!foundAgent) foundAgent = tree.standaloneAgents.get(senderId)
        if (!foundAgent) foundAgent = tree.agentIdToAgent.get(senderId)
        if (!foundAgent) {
          for (const [aid, agent] of tree.agentIdToAgent) {
            if (aid.startsWith(senderId + '@')) { foundAgent = agent; break }
          }
        }
        // Proof-of-life for an agent whose TeamCreate/Agent tool_use got
        // wiped on replay→live — materialize a standalone entry so the
        // sidebar reflects the agent that CLI is actively talking with.
        if (!foundAgent) {
          foundAgent = {
            id: `synthetic-${senderId}-${Date.now()}`,
            name: senderId,
            inputName: senderId,
            description: '',
            status: 'running',
            messages: [],
            lastSeenAt: entry.timestamp,
          }
          tree.standaloneAgents.set(senderId, foundAgent)
          changed = true
        }
        if (foundAgent) {
          foundAgent.messages.push({ from: senderId, text: msgText.substring(0, 500), ts: Date.now() })
          foundAgent.lastSeenAt = entry.timestamp || foundAgent.lastSeenAt
          changed = true
          // Two distinct ends: a teammate FINISHED its task (idle_notification,
          // idleReason "available") → done; the lead KICKED it (shutdown/terminated)
          // → terminated. Splitting them is what the "was it disbanded / kicked?"
          // status needs. Without idle_notification a teammate never left "running"
          // once the premature spawn-time done was removed above.
          const isKicked = msgType === 'shutdown_approved' || msgType === 'shutdown_response'
            || msgType === 'teammate_terminated' || msgBody.includes('shutdown_approved')
          if (isKicked) foundAgent.status = 'terminated'
          else if (msgType === 'idle_notification' && foundAgent.status === 'running') foundAgent.status = 'done'
        }
      }
    }

    if (entry.type === 'system') {
      const subtype = entry.subtype
      if (subtype === 'task_started' && entry.task_type === 'in_process_teammate') {
        const taskId = entry.task_id
        const desc = entry.description || ''
        if (taskId) {
          let matched: AgentInfo | undefined
          for (const team of tree.teams.values()) {
            for (const agent of team.agents.values()) {
              if (agent.status === 'running' && !agent.taskId) {
                if (!matched || (desc && agent.description.includes(desc))) matched = agent
              }
            }
          }
          if (!matched) {
            for (const agent of tree.standaloneAgents.values()) {
              if (agent.status === 'running' && !agent.taskId) {
                if (!matched || (desc && agent.description.includes(desc))) matched = agent
              }
            }
          }
          if (matched) { matched.taskId = taskId; tree.taskIdToAgent.set(taskId, matched) }
        }
      }
      if (subtype === 'task_notification') {
        const taskId = entry.task_id
        const status = entry.status
        if (taskId && status) {
          const agent = tree.taskIdToAgent.get(taskId)
          if (agent && agent.status === 'running') {
            agent.status = (status === 'failed' || status === 'stopped') ? 'error' : 'done'
            changed = true
          }
        }
      }
    }
  }

  if (changed) {
    // Trigger signal update
    tabAgentTrees.value = new Map(tabAgentTrees.value)
  }
  return changed
}

/** Build TreeNode children from agent tree for a given tab.
 *  Only agents confirmed running are returned. During JSONL replay
 *  (tabJsonlLive does not yet contain the tabId) nothing is shown —
 *  we wait for replay to finish, then we only keep agents with positive
 *  proof-of-life (no terminal signal yet). */
export function buildAgentTreeNodes(tabId: string): TreeNode[] {
  if (!tabJsonlLive.value.has(tabId)) return []
  const tree = tabAgentTrees.value.get(tabId)
  if (!tree) return []

  const children: TreeNode[] = []
  for (const [teamName, team] of tree.teams) {
    const liveAgents = [...team.agents.entries()].filter(([, a]) => a.status === 'running')
    if (liveAgents.length === 0) continue
    const teamNode: TreeNode = {
      id: `team-${teamName}`,
      type: 'team',
      label: teamName,
      status: 'busy',
      children: [],
    }
    for (const [agentName, agent] of liveAgents) {
      teamNode.children.push({
        id: `agent-${teamName}-${agentName}`,
        type: 'teammate',
        label: agentName,
        status: 'busy',
        children: [],
      })
    }
    children.push(teamNode)
  }
  for (const [name, agent] of tree.standaloneAgents) {
    if (agent.status !== 'running') continue
    children.push({
      id: `agent-${name}`,
      type: 'agent',
      label: name,
      status: 'busy',
      children: [],
    })
  }
  return children
}

/** Merge JSONL-derived agent nodes into the server tree */
export function mergeAgentTree(serverTree: TreeNode[]): TreeNode[] {
  return serverTree.map(node => {
    const agentChildren = buildAgentTreeNodes(node.id)
    if (agentChildren.length > 0) {
      return { ...node, children: [...node.children, ...agentChildren] }
    }
    return node
  })
}

const cleanupTimers = new Map<string, ReturnType<typeof setTimeout>>()

/** Wipe the agent tree entirely when JSONL transitions replay→live.
 *  Historical TeamCreate/Agent tool_use entries carry no liveness signal,
 *  so we drop everything and let genuine post-replay entries
 *  (teammate-message, new TeamCreate, task_started) re-populate
 *  only the agents that are actually alive right now. */
export function markAllAgentsExited(tabId: string): boolean {
  // БЫЛО: полный clear — вместе с давно мёртвыми выбрасывал ЖИВЫХ (агенты
  // переживают рестарт клиента в PTY; инцидент «работают, а Active 0»).
  // ТЕПЕРЬ: реплей уже довёл завершённых до done (пары закрыты) — отправляем
  // их в history немедленно, running оставляем; ложный running при мёртвом
  // PTY снимет staleness-прунер ниже.
  const tree = tabAgentTrees.value.get(tabId)
  if (!tree) return false
  let changed = false
  for (const [teamName, team] of tree.teams) {
    if (team.status === 'disbanded') continue
    for (const [agentName, agent] of team.agents) {
      if (agent.status !== 'running') {
        recordAgentHistory(tabId, agent)
        team.agents.delete(agentName)
        changed = true
      }
    }
    if (team.agents.size === 0) { tree.teams.delete(teamName); changed = true }
  }
  for (const [name, agent] of tree.standaloneAgents) {
    if (agent.status !== 'running') {
      recordAgentHistory(tabId, agent)
      tree.standaloneAgents.delete(name)
      changed = true
    }
  }
  tree.pendingAgentCalls.clear()
  scheduleStalePrune(tabId)
  if (changed) tabAgentTrees.value = new Map(tabAgentTrees.value)
  return changed
}

/** Обновить lastSeenAt агента по имени (пульс от потока subagent-записей). */
export function touchAgentAlive(tabId: string, agentName: string): void {
  const tree = tabAgentTrees.value.get(tabId)
  if (!tree) return
  const base = agentName.split('@')[0].replace(/-\d+$/, '')
  const a = findAgentByName(tree, agentName) ?? findAgentByName(tree, base)
  if (a) a.lastSeenAt = new Date().toISOString()
}

/** Running-агент без единой новой записи столько времени считается умершим
 *  вместе с прежним PTY (resume мёртвой сессии оставляет незакрытые пары). */
// 15 мин: длинные background-агенты думают дольше 3 минут между записями
// своего файла — прежний потолок ложно гасил живых (прод-репорт).
const STALE_RUNNING_MS = 900_000
const stalePruneTimers = new Map<string, ReturnType<typeof setTimeout>>()

function scheduleStalePrune(tabId: string): void {
  if (stalePruneTimers.has(tabId)) return
  const startedAt = Date.now()
  const timer = setTimeout(() => {
    stalePruneTimers.delete(tabId)
    const tree = tabAgentTrees.value.get(tabId)
    if (!tree) return
    let changed = false
    const stale = (a: AgentInfo): boolean => {
      if (a.status !== 'running') return false
      const last = a.lastSeenAt ? Date.parse(a.lastSeenAt) : 0
      // Ни одной записи после перехода в live И последняя известная старше
      // порога — агент не подаёт признаков жизни.
      return (Number.isNaN(last) ? 0 : last) < startedAt
    }
    for (const team of tree.teams.values()) {
      for (const a of team.agents.values()) {
        if (stale(a)) { a.status = 'done'; changed = true }
      }
    }
    for (const a of tree.standaloneAgents.values()) {
      if (stale(a)) { a.status = 'done'; changed = true }
    }
    if (changed) {
      tabAgentTrees.value = new Map(tabAgentTrees.value)
      scheduleCleanup(tabId) // унесёт свежепомеченных done в history
    }
  }, STALE_RUNNING_MS)
  stalePruneTimers.set(tabId, timer)
}

/** Schedule removal of done/error agents from tree after 5s */
export function scheduleCleanup(tabId: string): void {
  if (cleanupTimers.has(tabId)) return
  const timer = setTimeout(() => {
    cleanupTimers.delete(tabId)
    const tree = tabAgentTrees.value.get(tabId)
    if (!tree) return
    let changed = false
    for (const [teamName, team] of tree.teams) {
      // A DISBANDED team is kept whole (members + statuses) so the panel can show
      // the dissolved team as a tree — don't prune it.
      if (team.status === 'disbanded') continue
      for (const [agentName, agent] of team.agents) {
        if (agent.status !== 'running') { // done | error | terminated
          recordAgentHistory(tabId, agent) // keep it for the Agents panel's history
          team.agents.delete(agentName)
          changed = true
        }
      }
      if (team.agents.size === 0) { tree.teams.delete(teamName); changed = true }
    }
    for (const [name, agent] of tree.standaloneAgents) {
      if (agent.status !== 'running') {
        recordAgentHistory(tabId, agent)
        tree.standaloneAgents.delete(name)
        changed = true
      }
    }
    if (changed) {
      tabAgentTrees.value = new Map(tabAgentTrees.value)
      // Re-merge into session tree
      if (sessionTree.value) {
        sessionTree.value = mergeAgentTree(sessionTree.value)
      }
    }
  }, 5000)
  cleanupTimers.set(tabId, timer)
}
