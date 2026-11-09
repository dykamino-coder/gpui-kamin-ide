import type { JSX } from 'preact'
import { createPortal } from 'preact/compat'
import { useEffect, useLayoutEffect, useRef, useState } from 'preact/hooks'
import './CommandPalette.css'

export interface WorkspaceCommand {
  id: string
  title: string
  category?: string
  run(): void | Promise<unknown>
}

// Реестр и доступность команд принадлежат оболочке; палитра только выбирает действие.
export function CommandPalette({ commands }: { commands: WorkspaceCommand[] }): JSX.Element | null {
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [selected, setSelected] = useState(0)
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const pending = useRef(false)
  const input = useRef<HTMLInputElement>(null)
  const panel = useRef<HTMLDivElement>(null)
  const opener = useRef<HTMLElement | null>(null)
  const needle = query.trim().toLocaleLowerCase()
  const filtered = commands.filter(command => !command.id.startsWith('_') && command.id !== 'setContext'
    && [command.title, command.id, command.category ?? ''].some(value => value.toLocaleLowerCase().includes(needle)))
  const rows = filtered.slice(0, 50)
  const active = Math.min(selected, Math.max(0, rows.length - 1))

  function dismiss() {
    if (panel.current?.contains(document.activeElement)) opener.current?.focus()
    setOpen(false)
  }

  async function execute(command: WorkspaceCommand | undefined) {
    if (!command || pending.current) return
    pending.current = true
    setBusy(true)
    setError('')
    try {
      await command.run()
      dismiss()
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Command failed')
    } finally {
      pending.current = false
      setBusy(false)
    }
  }

  useEffect(() => {
    const show = () => {
      if (panel.current?.contains(document.activeElement)) { input.current?.focus(); return }
      opener.current = document.activeElement instanceof HTMLElement ? document.activeElement : null
      setQuery('')
      setSelected(0)
      setError('')
      setOpen(true)
    }
    const shortcut = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || !event.shiftKey || event.key.toLowerCase() !== 'p') return
      event.preventDefault()
      event.stopImmediatePropagation()
      if (!event.repeat) show()
    }
    window.addEventListener('workspace:command-palette', show)
    document.addEventListener('keydown', shortcut, true)
    return () => {
      window.removeEventListener('workspace:command-palette', show)
      document.removeEventListener('keydown', shortcut, true)
    }
  }, [])

  useLayoutEffect(() => {
    if (open) input.current?.focus()
  }, [open])

  useLayoutEffect(() => {
    if (!open) return
    // Escape в модальном окне не должен прерывать работающего агента.
    const keydown = (event: KeyboardEvent) => {
      if (event.isComposing || !['Escape', 'Enter', 'ArrowDown', 'ArrowUp', 'Tab'].includes(event.key)) return
      event.preventDefault()
      event.stopImmediatePropagation()
      if (event.key === 'Escape') dismiss()
      if (event.key === 'Enter' && !event.repeat) void execute(rows[active])
      if (event.key === 'Tab') input.current?.focus()
      if (event.key === 'ArrowDown') setSelected(rows.length ? (active + 1) % rows.length : 0)
      if (event.key === 'ArrowUp') setSelected(rows.length ? (active + rows.length - 1) % rows.length : 0)
    }
    document.addEventListener('keydown', keydown, true)
    return () => document.removeEventListener('keydown', keydown, true)
  }, [open, rows, active])

  useLayoutEffect(() => {
    panel.current?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: 'nearest' })
  }, [active, query])

  if (!open) return null
  return createPortal(
    <div class="workspace-palette-scrim" onMouseDown={event => {
      if (event.target === event.currentTarget) { event.preventDefault(); dismiss() }
    }}>
      <div ref={panel} class="workspace-palette" role="dialog" aria-modal="true" aria-label="Command palette">
        <div class="workspace-palette-input-row">
          <i class="fa-solid fa-magnifying-glass" aria-hidden="true" />
          <input ref={input} role="combobox" aria-label="Type a command" aria-autocomplete="list"
            aria-expanded="true" aria-controls="workspace-command-list" aria-activedescendant={rows.length ? `workspace-command-${active}` : undefined}
            placeholder="Type a command..." value={query} onInput={event => { setQuery(event.currentTarget.value); setSelected(0); setError('') }} />
          <kbd>Esc</kbd>
        </div>
        <div id="workspace-command-list" class="workspace-palette-list" role="listbox" aria-label="Commands" aria-busy={busy}>
          {rows.map((command, index) => <div key={command.id} id={`workspace-command-${index}`} role="option"
            aria-selected={active === index} aria-disabled={busy} class="workspace-palette-command"
            onMouseEnter={() => setSelected(index)} onMouseDown={event => event.preventDefault()} onClick={() => void execute(command)}>
            <span class="workspace-palette-title">{command.category && <span class="workspace-palette-category">{command.category}: </span>}{command.title}</span>
            <span class="workspace-palette-id">{command.id}</span>
          </div>)}
          {!rows.length && <div class="workspace-palette-empty">No commands match "{query}"</div>}
        </div>
        {error && <div class="workspace-palette-error" role="alert">{error}</div>}
        <div class="workspace-palette-footer">{filtered.length} command{filtered.length === 1 ? '' : 's'} · {busy ? 'Running...' : 'Enter to run'}</div>
      </div>
    </div>, document.body,
  )
}
