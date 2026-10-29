import { useState } from 'preact/hooks'
import { commandSnapshots, commandOutput } from '../signals/commands'
import { ptySessions } from '../signals/server'
import { getSessionToken } from '../signals/auth'
import { wsClient } from '../services/ws-client'

export function RunningCommands() {
  const [sessionId, setSessionId] = useState('')
  const [selected, setSelected] = useState<string | null>(null)
  const sessions = ptySessions.value
  const active = sessions.some((s) => s.id === sessionId) ? sessionId : sessions[0]?.id || ''
  const commands = commandSnapshots.value.find((s) => s.sessionId === active)?.commands ?? []
  const action = (type: string, id: string) =>
    wsClient.send({ type: `commands:${type}`, token: getSessionToken(), sessionId: active, id })
  const result = commandOutput.value
  return (
    <section
      style={{ padding: '16px', marginBottom: '16px', border: '1px solid var(--border-primary)', borderRadius: '8px' }}
    >
      <h3>Running commands</h3>
      <select
        aria-label="Command session"
        value={active}
        onChange={(e) => {
          setSessionId(e.currentTarget.value)
          setSelected(null)
        }}
      >
        {sessions.map((s) => (
          <option key={s.id} value={s.id}>
            {s.sessionTitle || s.id}
          </option>
        ))}
      </select>
      {commands.length === 0 && <p>No running commands</p>}
      {commands.map((c) => (
        <div key={c.id} style={{ borderBottom: '1px solid var(--border-primary)', padding: '10px 0' }}>
          <div>
            <b>{c.tool}</b> · {c.background ? 'Background' : 'Foreground'} · {c.status} ·{' '}
            {Math.max(0, Math.floor((Date.now() - c.startedAt) / 1000))}s
          </div>
          <code
            style={{
              display: 'block',
              maxWidth: '100%',
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
            }}
            title={c.command}
          >
            {c.command}
          </code>
          <button disabled={c.status !== 'running'} onClick={() => action('stop', c.id)}>
            Stop
          </button>{' '}
          <button
            onClick={() => {
              setSelected(c.id)
              action('output', c.id)
            }}
          >
            Open output
          </button>
        </div>
      ))}
      {selected && (
        <div>
          <button onClick={() => action('output', selected)}>Refresh output</button>{' '}
          <button onClick={() => setSelected(null)}>Close output</button>
          <pre style={{ maxHeight: '300px', overflow: 'auto', whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>
            {result?.sessionId === active && result.id === selected ? result.output : 'Loading…'}
          </pre>
        </div>
      )}
    </section>
  )
}
