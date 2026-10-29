import type { JSX } from 'preact'
import { createPortal } from 'preact/compat'
import { useEffect, useRef, useState } from 'preact/hooks'
import { useBridge } from '../../hooks/useBridge'
import type { RunningCommand } from '../../../shared/commands'
import './commands.css'

export function CommandsButton({ tabId }: { tabId: string }): JSX.Element {
  const bridge = useBridge()
  const root = useRef<HTMLDivElement>(null)
  const popover = useRef<HTMLDivElement>(null)
  const [position, setPosition] = useState({ top: 0, left: 8 })
  const [open, setOpen] = useState(false)
  const [commands, setCommands] = useState<RunningCommand[]>([])
  const [selected, setSelected] = useState<string | null>(null)
  const [output, setOutput] = useState<{ id: string; output: string } | null>(null)
  const [updated, setUpdated] = useState(0)
  const [serverAvailable, setServerAvailable] = useState(false)
  useEffect(() => {
    setCommands([])
    setOpen(false)
    setSelected(null)
    setOutput(null)
    setServerAvailable(false)
    const off = bridge.onCommandsSnapshot((id, snapshot) => {
      if (id !== tabId) return
      setCommands(snapshot.commands)
      setServerAvailable(snapshot.serverAvailable !== false)
      setUpdated(Date.now())
    })
    const offOutput = bridge.onCommandOutput((id, result) => {
      if (id === tabId) {
        setOutput(result)
      }
    })
    bridge.commandAction(tabId, 'list')
    // Older servers never request client shell reports or publish snapshots.
    // Poll the local registry too, including while the popover is closed (badge).
    const timer = setInterval(() => bridge.commandAction(tabId, 'list'), 1000)
    return () => {
      off()
      offOutput()
      clearInterval(timer)
    }
  }, [bridge, tabId])
  useEffect(() => {
    if (!open) return
    const place = () => {
      const rect = root.current?.getBoundingClientRect()
      if (rect)
        setPosition({
          top: rect.bottom + 6,
          left: Math.max(
            8,
            Math.min(
              rect.right - Math.min(420, window.innerWidth - 32),
              window.innerWidth - Math.min(420, window.innerWidth - 32) - 8,
            ),
          ),
        })
    }
    place()
    window.addEventListener('resize', place)
    const close = (event: MouseEvent) => {
      if (!root.current?.contains(event.target as Node) && !popover.current?.contains(event.target as Node))
        setOpen(false)
    }
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false)
    }
    document.addEventListener('mousedown', close)
    document.addEventListener('keydown', escape)
    return () => {
      window.removeEventListener('resize', place)
      document.removeEventListener('mousedown', close)
      document.removeEventListener('keydown', escape)
    }
  }, [open])
  useEffect(() => {
    if (open && selected) bridge.commandAction(tabId, 'output', selected)
  }, [bridge, tabId, open, selected, updated])
  const background = commands.filter((c) => c.background).length
  return (
    <div class="commands-control" ref={root}>
      <button
        type="button"
        class="header-btn commands-toggle"
        data-tooltip="Running commands"
        aria-label="Running commands"
        aria-expanded={open}
        aria-haspopup="dialog"
        onClick={() => setOpen(!open)}
      >
        <i class="fas fa-terminal" />
        {background > 0 && <span class="commands-badge">{background}</span>}
      </button>
      {open &&
        createPortal(
          <div
            class="commands-popover"
            ref={popover}
            style={{ top: position.top, left: position.left }}
            role="dialog"
            aria-label="Running commands"
          >
            <div class="commands-heading">
              Running commands{' '}
              <button type="button" aria-label="Close commands" onClick={() => setOpen(false)}>
                ×
              </button>
            </div>
            {!serverAvailable && (
              <p class="commands-empty">Server command list unavailable. Showing local background commands.</p>
            )}
            {serverAvailable && commands.length === 0 && <p class="commands-empty">No running commands</p>}
            {commands.map((c) => (
              <div key={c.id} class="commands-row">
                <div class="commands-meta">
                  <b>{c.tool}</b>
                  <span>
                    {c.background ? 'Background' : 'Foreground'} · {c.status} ·{' '}
                    {Math.max(0, Math.floor((Date.now() - c.startedAt) / 1000))}s
                  </span>
                </div>
                <code title={c.command}>{c.command}</code>
                <div class="commands-actions">
                  <button
                    type="button"
                    disabled={c.status !== 'running'}
                    onClick={() => bridge.commandAction(tabId, 'stop', c.id)}
                  >
                    Stop
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setOutput(null)
                      setSelected(c.id)
                      bridge.commandAction(tabId, 'output', c.id)
                    }}
                  >
                    Open output
                  </button>
                </div>
              </div>
            ))}
            {selected && (
              <div class="commands-output">
                <div class="commands-heading">
                  Output{' '}
                  <button type="button" aria-label="Close output" onClick={() => setSelected(null)}>
                    ×
                  </button>
                </div>
                <pre>{output?.id === selected ? output.output || 'No output yet.' : 'Loading…'}</pre>
              </div>
            )}
          </div>,
          document.body,
        )}
    </div>
  )
}
