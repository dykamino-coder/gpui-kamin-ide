import type { JSX } from 'preact'
import { useEffect, useRef, useState } from 'preact/hooks'
import type { RunningCommand } from '../../../shared/commands'
import { useBridge } from '../../hooks/useBridge'

export const isCommandActive = (command: RunningCommand): boolean =>
  ['running', 'stopping', 'disconnected'].includes(command.status)

export function commandDuration(command: RunningCommand, now: number): string {
  const seconds = Math.max(0, Math.floor(((command.finishedAt ?? now) - command.startedAt) / 1000))
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  return `${hours ? `${hours}h ` : ''}${hours || minutes ? `${minutes}m ` : ''}${seconds % 60}s`
}

function commandStatus(command: RunningCommand): string {
  if (command.status === 'completed') return 'Completed'
  if (command.status === 'stopped') return 'Stopped'
  if (command.status === 'failed') return command.exitCode != null ? `Failed (exit ${command.exitCode})` : 'Failed'
  if (command.status === 'stopping') return 'Stopping…'
  if (command.status === 'disconnected') return 'Reconnecting…'
  return 'Running'
}

// Mounted only while a card is visible and expanded. Output reads do not consume
// the agent's BashOutput cursor; closing the card stops both polling and events.
export function CommandOutput({ tabId, id }: { tabId: string; id: string }): JSX.Element {
  const bridge = useBridge()
  const [output, setOutput] = useState('')
  const tail = useRef<HTMLPreElement>(null)
  useEffect(() => {
    setOutput('')
    const off = bridge.onCommandOutput((tab, result) => {
      if (tab === tabId && result.id === id) setOutput(result.output)
    })
    const read = () => bridge.commandAction(tabId, 'output', id)
    read()
    const timer = setInterval(read, 1000)
    return () => {
      off()
      clearInterval(timer)
    }
  }, [bridge, tabId, id])
  useEffect(() => {
    if (tail.current) tail.current.scrollTop = tail.current.scrollHeight
  }, [output])
  return (
    <pre class={`commands-tail${output ? '' : ' commands-muted'}`} ref={tail}>
      {output || 'No output yet'}
    </pre>
  )
}

export function CommandCard({
  command,
  tabId,
  now,
}: {
  command: RunningCommand
  tabId: string
  now: number
}): JSX.Element {
  const bridge = useBridge()
  const [expanded, setExpanded] = useState(false)
  const active = isCommandActive(command)
  const title = command.description?.trim() || command.command.replace(/\s+/g, ' ').trim() || command.tool
  const status = commandStatus(command)
  return (
    <article class={`commands-card commands-${command.status}`}>
      <div class="commands-card-row">
        <button
          type="button"
          class="commands-disclosure"
          aria-expanded={expanded}
          aria-label={`${expanded ? 'Collapse' : 'Expand'} task: ${title}`}
          onClick={() => setExpanded(!expanded)}
        >
          <i class={`fas fa-caret-${expanded ? 'down' : 'right'} commands-chevron`} aria-hidden="true" />
          <span class="commands-title" title={title}>
            {title}
          </span>
          <span class="commands-timing">
            <span class="commands-duration">{commandDuration(command, now)}</span>
            {command.status !== 'running' && <span class="commands-status">{status}</span>}
          </span>
        </button>
        {active && (
          <button
            type="button"
            class="commands-icon commands-stop"
            aria-label={`Stop task: ${title}`}
            title="Stop task"
            disabled={command.status !== 'running'}
            onClick={() => bridge.commandAction(tabId, 'stop', command.id)}
          >
            <i class="fas fa-stop" aria-hidden="true" />
          </button>
        )}
      </div>
      {expanded && (
        <div class="commands-details">
          <pre class="commands-command">&gt; {command.command}</pre>
          <CommandOutput tabId={tabId} id={command.id} />
        </div>
      )}
    </article>
  )
}
