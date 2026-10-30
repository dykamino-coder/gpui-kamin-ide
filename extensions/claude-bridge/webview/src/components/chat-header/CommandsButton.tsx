import type { JSX } from 'preact'
import { createPortal } from 'preact/compat'
import { useEffect, useRef, useState } from 'preact/hooks'
import { useBridge } from '../../hooks/useBridge'
import type { RunningCommand } from '../../../shared/commands'
import { CommandCard, isCommandActive } from './CommandCard'
import './commands.css'

export function CommandsButton({ tabId }: { tabId: string }): JSX.Element {
  const bridge = useBridge()
  const root = useRef<HTMLDivElement>(null)
  const popover = useRef<HTMLDivElement>(null)
  const [position, setPosition] = useState({
    top: 0,
    left: 8,
    width: 520,
    maxHeight: 520,
  })
  const [open, setOpen] = useState(false)
  const [expanded, setExpanded] = useState(false)
  const [runningOpen, setRunningOpen] = useState(true)
  const [completedOpen, setCompletedOpen] = useState(false)
  const [commands, setCommands] = useState<RunningCommand[]>([])
  const [now, setNow] = useState(Date.now())
  const [serverAvailable, setServerAvailable] = useState(false)
  const close = () => {
    setOpen(false)
    root.current?.querySelector('button')?.focus()
  }
  useEffect(() => {
    setCommands([])
    setOpen(false)
    setExpanded(false)
    setRunningOpen(true)
    setCompletedOpen(false)
    setServerAvailable(false)
    const off = bridge.onCommandsSnapshot((id, snapshot) => {
      if (id !== tabId) return
      setCommands(snapshot.commands)
      setServerAvailable(snapshot.serverAvailable !== false)
    })
    const read = () => {
      bridge.commandAction(tabId, 'list')
      setNow(Date.now())
    }
    read()
    // Poll the local registry even with older servers and a closed popover (badge).
    const timer = setInterval(read, 1000)
    return () => {
      off()
      clearInterval(timer)
    }
  }, [bridge, tabId])
  useEffect(() => {
    if (!open) return
    const place = () => {
      const rect = root.current?.getBoundingClientRect()
      if (!rect) return
      const panel = root.current?.closest('.chat-panel')?.getBoundingClientRect()
      const left = Math.max(8, (panel?.left ?? 0) + 8)
      const right = Math.min(window.innerWidth - 8, (panel?.right ?? window.innerWidth) - 8)
      const top = Math.max(8, rect.bottom + 6)
      const available = Math.max(0, Math.min(window.innerHeight, panel?.bottom ?? window.innerHeight) - top - 8)
      const width = Math.max(0, Math.min(expanded ? right - left : 520, right - left))
      setPosition({
        top,
        left: Math.max(left, Math.min(rect.right - width, right - width)),
        width,
        maxHeight: expanded ? available : Math.min(520, available),
      })
    }
    place()
    window.addEventListener('resize', place)
    const observer = new ResizeObserver(place)
    const panel = root.current?.closest('.chat-panel')
    if (panel) observer.observe(panel)
    const outside = (event: MouseEvent) => {
      if (!root.current?.contains(event.target as Node) && !popover.current?.contains(event.target as Node))
        setOpen(false)
    }
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault()
        event.stopPropagation()
        close()
      }
    }
    document.addEventListener('mousedown', outside)
    document.addEventListener('keydown', escape)
    return () => {
      observer.disconnect()
      window.removeEventListener('resize', place)
      document.removeEventListener('mousedown', outside)
      document.removeEventListener('keydown', escape)
    }
  }, [open, expanded])
  useEffect(() => {
    if (open) popover.current?.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus()
  }, [open])
  const running = commands.filter(isCommandActive)
  const completed = commands.filter((command) => !isCommandActive(command))
  return (
    <div class="commands-control" ref={root}>
      <button
        type="button"
        class="header-btn commands-toggle"
        data-tooltip="Background tasks"
        aria-label="Background tasks"
        aria-expanded={open}
        aria-haspopup="dialog"
        onClick={() => setOpen(!open)}
      >
        <i class="fas fa-terminal" aria-hidden="true" />
        {running.length > 0 && <span class="commands-badge">{running.length}</span>}
      </button>
      {open &&
        createPortal(
          <div
            class={`commands-popover${expanded ? ' commands-expanded' : ''}`}
            ref={popover}
            style={{
              ...position,
              ...(expanded ? { height: position.maxHeight } : {}),
            }}
            role="dialog"
            aria-label="Background tasks"
          >
            <div class="commands-heading">
              <span>Background tasks</span>
              <div class="commands-toolbar">
                <button
                  type="button"
                  class="commands-icon"
                  aria-label="Clear completed tasks"
                  title="Clear completed tasks"
                  disabled={completed.length === 0}
                  onClick={() => bridge.commandAction(tabId, 'clear')}
                >
                  <i class="far fa-trash-can" aria-hidden="true" />
                </button>
                <button
                  type="button"
                  class="commands-icon"
                  aria-label={expanded ? 'Restore popover size' : 'Expand popover'}
                  title={expanded ? 'Restore popover size' : 'Expand popover'}
                  aria-pressed={expanded}
                  onClick={() => setExpanded(!expanded)}
                >
                  <i class={`fas fa-${expanded ? 'compress' : 'expand'}`} aria-hidden="true" />
                </button>
                <button
                  type="button"
                  class="commands-icon"
                  aria-label="Close background tasks"
                  title="Close"
                  onClick={close}
                >
                  <i class="fas fa-xmark" aria-hidden="true" />
                </button>
              </div>
            </div>
            <div class="commands-body">
              {!serverAvailable && (
                <p class="commands-note">
                  Local tasks · Server tasks will appear when connected to a supported server.
                </p>
              )}
              <section class="commands-section" aria-label="Running tasks">
                <button
                  type="button"
                  class="commands-section-toggle"
                  aria-expanded={runningOpen}
                  onClick={() => setRunningOpen(!runningOpen)}
                >
                  <i class={`fas fa-caret-${runningOpen ? 'down' : 'right'}`} aria-hidden="true" />
                  Running <span>{running.length}</span>
                </button>
                {runningOpen &&
                  (running.length ? (
                    running.map((command) => <CommandCard key={command.id} command={command} tabId={tabId} now={now} />)
                  ) : (
                    <p class="commands-empty">No running tasks</p>
                  ))}
              </section>
              <section class="commands-section" aria-label="Completed tasks">
                <button
                  type="button"
                  class="commands-section-toggle"
                  aria-expanded={completedOpen}
                  onClick={() => setCompletedOpen(!completedOpen)}
                >
                  <i class={`fas fa-caret-${completedOpen ? 'down' : 'right'}`} aria-hidden="true" />
                  Completed <span>{completed.length}</span>
                </button>
                {completedOpen &&
                  (completed.length ? (
                    completed.map((command) => (
                      <CommandCard key={command.id} command={command} tabId={tabId} now={now} />
                    ))
                  ) : (
                    <p class="commands-empty">No completed tasks</p>
                  ))}
              </section>
            </div>
          </div>,
          document.body,
        )}
    </div>
  )
}
