import { useRef, useState } from 'preact/hooks'
import { Modal } from '../shared'
import type { RequestLogEntry } from '../../types'
import styles from './ToolsModal.module.css'

interface ToolDef {
  name: string
  description?: string
  input_schema?: Record<string, unknown>
}

interface ToolsModalProps {
  request: RequestLogEntry | null
  onClose: () => void
  filter?: 'tools' | 'mcp'
}

export function ToolsModal({ request, onClose, filter = 'tools' }: ToolsModalProps) {
  const contentRef = useRef<HTMLDivElement>(null)
  const [activeNav, setActiveNav] = useState<string | null>(null)

  if (!request) return null

  const isMcp = (name: string) => name.startsWith('mcp__')
  const allBodyTools: ToolDef[] = (request.requestBody as any)?.tools ?? []
  const bodyTools = allBodyTools.filter(t => filter === 'mcp' ? isMcp(t.name) : !isMcp(t.name))
  const filteredUsed = request.toolsUsed.filter(n => filter === 'mcp' ? isMcp(n) : !isMcp(n))
  const usedSet = new Set(filteredUsed)

  const rawTools: ToolDef[] = bodyTools.length > 0
    ? bodyTools
    : filteredUsed.map(name => ({ name }))

  // Sort alphabetically
  const tools = [...rawTools].sort((a, b) => a.name.localeCompare(b.name))
  const hasTools = tools.length > 0

  function scrollToTool(name: string) {
    setActiveNav(name)
    const container = contentRef.current
    if (!container) return
    const target = container.querySelector(`[data-tool="${CSS.escape(name)}"]`) as HTMLElement | null
    if (!target) return
    const relativeTop = target.getBoundingClientRect().top - container.getBoundingClientRect().top + container.scrollTop
    container.scrollTo({ top: relativeTop - 8, behavior: 'smooth' })
  }

  return (
    <Modal
      isOpen
      onClose={onClose}
      title={`${filter === 'mcp' ? 'MCP Tools' : 'Tools'} — Request #${request.id.slice(0, 8)}`}
      size="lg"
    >
      {hasTools ? (
        <div class={styles.layout}>
          {/* Left nav */}
          <div class={styles.nav}>
            <div class={styles.navStats}>
              {tools.length} tools{usedSet.size > 0 && ` · ${usedSet.size} used`}
            </div>
            {tools.map((t) => (
              <button
                key={t.name}
                class={`${styles.navItem} ${activeNav === t.name ? styles.navActive : ''} ${usedSet.has(t.name) ? styles.navUsed : ''}`}
                onClick={() => scrollToTool(t.name)}
                title={t.name}
              >
                <span class={styles.navName}>{t.name}</span>
                {usedSet.has(t.name) && <span class={styles.navDot} />}
              </button>
            ))}
          </div>

          {/* Right content */}
          <div class={styles.content} ref={contentRef}>
            {tools.map((t) => (
              <div
                key={t.name}
                data-tool={t.name}
                class={`${styles.item} ${usedSet.has(t.name) ? styles.used : ''}`}
              >
                <div class={styles.nameRow}>
                  <code class={styles.name}>{t.name}</code>
                  {usedSet.has(t.name) && <span class={styles.usedBadge}>used</span>}
                </div>
                {t.description && <p class={styles.desc}>{t.description}</p>}
                {t.input_schema && (
                  <pre class={styles.schema}>{JSON.stringify(t.input_schema, null, 2)}</pre>
                )}
              </div>
            ))}
          </div>
        </div>
      ) : (
        <div class={styles.empty}>No tools available in this request.</div>
      )}
    </Modal>
  )
}
