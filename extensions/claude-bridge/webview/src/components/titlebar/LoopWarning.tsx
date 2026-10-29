import { useEffect, useState } from 'preact/hooks'
import type { KaminBridgeApi } from '../../../shared/types'
import { loopObservations } from '../../signals/loop-observation'
import { tabJsonlLive } from '../../signals/agents'
import { archivedViewTs } from '../../signals/jsonl'
import { activeSegmentTsByTab } from '../../signals/compact-segments'
import { tabWaiting, tabActivity } from '../../signals/ui'
import { tabs } from '../../signals/tabs'
import { activeWidgets } from '../../signals/widgets'
import { loopSignal } from '../../utils/loop-observation'
import styles from './LoopWarning.module.css'

interface Props {
  bridge: KaminBridgeApi
  tabId: string
  connected: boolean
  working: boolean
}

// Keep connection/activity subscriptions out of the chat root.
export function LiveLoopWarning({ bridge, tabId }: Pick<Props, 'bridge' | 'tabId'>) {
  const connected = tabs.value.find((t) => t.id === tabId)?.status === 'connected'
  const working = !!tabActivity.value.get(tabId)?.isWorking
  return <LoopWarning bridge={bridge} tabId={tabId} connected={connected} working={working} />
}

export function LoopWarning({ bridge, tabId, connected, working }: Props) {
  const [now, setNow] = useState(Date.now)
  const [dismissed, setDismissed] = useState<ReadonlySet<string>>(() => new Set())
  const observation = loopObservations.value.get(tabId)
  const waiting =
    tabWaiting.value.get(tabId) ||
    activeWidgets.value.some((w) => !w.resolved && (!w.data?.tabId || w.data.tabId === tabId))
  const live =
    connected &&
    working &&
    !waiting &&
    tabJsonlLive.value.has(tabId) &&
    !archivedViewTs(tabId) &&
    !activeSegmentTsByTab.value.has(tabId)
  useEffect(() => {
    if (!live) return
    setNow(Date.now())
    const timer = setInterval(() => setNow(Date.now()), 5000)
    return () => clearInterval(timer)
  }, [live, tabId, observation?.current?.key])
  const reason = loopSignal(observation?.current ?? null, observation?.baseline ?? null, now, live)
  const key = `${tabId}:${observation?.current?.key ?? ''}`
  if (!reason || dismissed.has(key)) return null
  const dismiss = () =>
    setDismissed((previous) => {
      const next = new Set(previous)
      next.add(key)
      if (next.size > 64) next.delete(next.values().next().value!)
      return next
    })
  const text =
    reason === 'repeated-errors'
      ? 'The same tool error has repeated at least 3 times.'
      : reason === 'repeated-calls'
        ? 'The same tool call has repeated at least 4 times.'
        : reason === 'many-calls'
          ? 'This turn has used over 2.5× the usual number of tools for this project.'
          : 'This turn has taken over 2.5× the usual time for this project.'
  return (
    <div class={styles.notice} role="status" aria-live="polite">
      <span>{text} You can interrupt if it seems stuck.</span>
      <button
        type="button"
        onClick={() => {
          dismiss()
          bridge.interruptSession(tabId)
        }}
      >
        Interrupt
      </button>
      <button type="button" aria-label="Dismiss loop notice for this turn" onClick={dismiss}>
        Dismiss
      </button>
    </div>
  )
}
