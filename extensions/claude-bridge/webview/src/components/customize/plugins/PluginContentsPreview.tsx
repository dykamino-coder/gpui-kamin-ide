import type { JSX } from 'preact'
import { useState, useEffect } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import styles from './PluginContentsPreview.module.css'

interface Counts {
  commands?: number
  agents?: number
  skills?: number
  mcpServers?: number
  lspServers?: number
  hooks?: number
}

interface Contents {
  skills: { name: string; description?: string }[]
  agents: { name: string; description?: string }[]
  commands: { name: string; description?: string }[]
  mcpServers: string[]
  hooks: { event: string; matcher?: string; type: string; summary: string }[]
}

/** Expandable preview of a plugin's actual contents — names + descriptions
 *  of every skill/agent/command/mcp-server it ships. Without this users
 *  see opaque counts ("3 skills · 2 agents") and have to install blind to
 *  find out whether the plugin contains what they want. Lazy-loaded:
 *  contents are fetched only when the user clicks to expand. */
export function PluginContentsPreview({ dir, counts }: { dir: string | undefined; counts: Counts }): JSX.Element | null {
  const bridge = useBridge()
  const [open, setOpen] = useState(false)
  const [contents, setContents] = useState<Contents | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Total items implied by counts — if zero, no point in showing the
  // expander toggle (PluginCard already filters this case but keep the
  // guard close to where it's used).
  const total =
    (counts.commands ?? 0) +
    (counts.agents ?? 0) +
    (counts.skills ?? 0) +
    (counts.mcpServers ?? 0) +
    (counts.hooks ?? 0)
  if (total <= 0) return null
  if (!dir) return null

  useEffect(() => {
    if (!open || contents !== null) return
    setLoading(true)
    setError(null)
    bridge.getPluginContents(dir).then(c => {
      if (c === null) {
        setError('Plugin not yet downloaded — install to see contents.')
      } else {
        setContents(c)
      }
    }).catch((err) => {
      setError(err?.message || 'Failed to load contents')
    }).finally(() => setLoading(false))
  }, [open, dir])

  return (
    <div class={styles.wrap}>
      <button
        type="button"
        class={styles.toggle}
        onClick={() => setOpen(!open)}
        title={open ? 'Hide contents' : 'Show contents'}
      >
        <i class={`fas fa-chevron-${open ? 'down' : 'right'}`} />
        {open ? 'Hide' : 'Preview'} contents
      </button>
      {open && (
        <div class={styles.body}>
          {loading && <div class={styles.muted}><i class="fas fa-spinner fa-spin" /> Loading…</div>}
          {error && <div class={styles.error}>{error}</div>}
          {contents && (
            <>
              <Section
                icon="fa-graduation-cap"
                color="var(--accent-purple)"
                label="Skills"
                items={contents.skills}
              />
              <Section
                icon="fa-user-robot"
                color="var(--accent-primary)"
                label="Agents"
                items={contents.agents}
              />
              <Section
                icon="fa-terminal"
                color="var(--accent-green)"
                label="Commands"
                prefix="/"
                items={contents.commands}
              />
              {contents.mcpServers.length > 0 && (
                <div class={styles.section}>
                  <div class={styles.sectionHeader} style="color:var(--accent-yellow)">
                    <i class="fas fa-plug" />
                    MCP servers ({contents.mcpServers.length})
                  </div>
                  <div class={styles.mcpList}>
                    {contents.mcpServers.map(name => (
                      <span key={name} class={styles.mcpBadge}>{name}</span>
                    ))}
                  </div>
                </div>
              )}
              {contents.hooks.length > 0 && (
                <div class={styles.section}>
                  <div class={styles.sectionHeader} style="color:var(--accent-orange)">
                    <i class="fas fa-anchor" />
                    Hooks ({contents.hooks.length})
                  </div>
                  <ul class={styles.itemList}>
                    {contents.hooks.map((h, i) => (
                      <li key={`${h.event}-${i}`} class={styles.item}>
                        <span class={styles.itemName} style="font-family:var(--font-mono);font-size:var(--fs-xs)">
                          <strong style="color:var(--accent-orange)">{h.event}</strong>
                          {h.matcher && <> · <code style="color:var(--text-secondary)">{h.matcher}</code></>}
                          <span style="color:var(--text-muted);margin-left:6px">[{h.type}]</span>
                        </span>
                        {h.summary && <span class={styles.itemDesc}>{truncate(h.summary, 140)}</span>}
                      </li>
                    ))}
                  </ul>
                </div>
              )}
              {contents.skills.length === 0
                && contents.agents.length === 0
                && contents.commands.length === 0
                && contents.mcpServers.length === 0
                && contents.hooks.length === 0 && (
                <div class={styles.muted}>No content found in this plugin directory.</div>
              )}
            </>
          )}
        </div>
      )}
    </div>
  )
}

function Section({
  icon, color, label, items, prefix,
}: {
  icon: string; color: string; label: string
  items: { name: string; description?: string }[]
  prefix?: string
}): JSX.Element | null {
  if (!items.length) return null
  return (
    <div class={styles.section}>
      <div class={styles.sectionHeader} style={`color:${color}`}>
        <i class={`fas ${icon}`} />
        {label} ({items.length})
      </div>
      <ul class={styles.itemList}>
        {items.map(it => (
          <li key={it.name} class={styles.item}>
            <span class={styles.itemName}>{prefix || ''}{it.name}</span>
            {it.description && (
              <span class={styles.itemDesc}>{truncate(it.description, 120)}</span>
            )}
          </li>
        ))}
      </ul>
    </div>
  )
}

function truncate(s: string, max: number): string {
  if (s.length <= max) return s
  return s.slice(0, max - 1) + '…'
}
