import type { JSX } from 'preact'
import { useMemo, useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { activeTabId } from '../../../signals/tabs'
import { showToast } from '../../../signals/toasts'
import { copyWithToast } from '../../../lib/clipboard'

interface BashInput {
  command?: string
  description?: string
  timeout?: number
  run_in_background?: boolean
}

const RESULT_PREVIEW_LINES = 30

/** Bash: render the command as a terminal-styled block with optional
 *  description callout above. Results colour `<stderr>` tags red and
 *  fold long stdout behind an expand toggle. */
export function BashToolRender({ input }: { input: BashInput }): JSX.Element {
  const bridge = useBridge()
  const command = input.command ?? ''

  function reRun(): void {
    const tabId = activeTabId.value
    if (!tabId || !command) return
    bridge.submitText(tabId, command)
    showToast({ type: 'info', title: 'Re-running', message: command.slice(0, 80), duration: 2500 })
  }

  function copyCommand(): void {
    if (!command) return
    void copyWithToast(command, 'Copied command')
  }

  function useAsInput(): void {
    if (!command) return
    const w = window as unknown as { __appendToInput?: (text: string) => void }
    w.__appendToInput?.(`\`\`\`bash\n${command}\n\`\`\``)
  }

  return (
    <div style="padding:6px 0">
      {input.description && (
        <div style="font-size:11px;color:var(--text-muted);padding:2px 2px 6px;font-style:italic">
          # {input.description}
        </div>
      )}
      <pre style="margin:0;padding:8px 12px;background:var(--bg-sidebar);color:var(--accent-green);font-family:var(--font-mono);font-size:12px;line-height:1.5;white-space:pre-wrap;word-break:break-word;border:1px solid var(--bg-surface);border-radius:var(--radius-xs);overflow-x:auto">
        <span style="color:var(--text-muted);user-select:none;margin-right:8px">$</span>
        {command}
      </pre>
      <div style="display:flex;gap:6px;margin-top:6px;align-items:center;flex-wrap:wrap">
        <BlockAction icon="fa-copy" label="Copy" onClick={copyCommand} disabled={!command} />
        <BlockAction icon="fa-rotate-right" label="Re-run" onClick={reRun} disabled={!command} title="Send the same command to the active session" />
        <BlockAction icon="fa-arrow-down-to-line" label="Use as input" onClick={useAsInput} disabled={!command} title="Paste this command into the input bar as a code block" />
        <span style="flex:1" />
        {input.timeout !== undefined && (
          <span style="font-size:10px;color:var(--text-muted)">
            <i class="fas fa-clock" style="margin-right:4px" />
            {Math.round(input.timeout / 1000)}s
          </span>
        )}
        {input.run_in_background && (
          <span style="font-size:10px;color:var(--accent-primary)">
            <i class="fas fa-forward" style="margin-right:4px" />
            background
          </span>
        )}
      </div>
    </div>
  )
}

/** Parse a bash tool-result string. CLI tags stderr via `<stderr>...`
 *  blocks and wraps the final exit-code announcement; returns a list of
 *  segments so we can colour them separately. Unwrapped text → stdout. */
// Hard cap on how much of a bash/pwsh result is MOUNTED into the DOM. Every
// other tool result caps (2000 chars), but this path used to render the FULL
// text — a multi-MB stdout (`cat huge.log`, a verbose build) built a giant DOM +
// re-ran parseBashResult/split over the whole string on every render and froze
// the shared renderer thread. We mount only the tail (errors/warnings cluster at
// the end); Copy / Share / Use-as-input still operate on the full text.
const MAX_RESULT_RENDER_CHARS = 200_000

export function BashResultRender({ text, isError }: { text: string; isError: boolean }): JSX.Element {
  const [expanded, setExpanded] = useState(false)
  // Memoise the cap + parse so a scroll/hover re-render doesn't re-split MBs.
  const { renderText, hiddenChars, totalLines, segments } = useMemo(() => {
    const capped = text.length > MAX_RESULT_RENDER_CHARS
    const rt = capped ? text.slice(-MAX_RESULT_RENDER_CHARS) : text
    return { renderText: rt, hiddenChars: text.length - rt.length, totalLines: rt.split('\n').length, segments: parseBashResult(rt) }
  }, [text])
  const shouldFold = !expanded && totalLines > RESULT_PREVIEW_LINES

  function copyOutput(): void {
    if (!text) return
    void copyWithToast(text, 'Copied output')
  }

  function shareAsMarkdown(): void {
    // Reconstruct command + output as a self-contained markdown block —
    // useful for pasting into a bug report, slack, or doc.
    const md = '```\n' + text + '\n```'
    void copyWithToast(md, 'Copied as markdown')
  }

  function useOutputAsInput(): void {
    if (!text) return
    const w = window as unknown as { __appendToInput?: (text: string) => void }
    // Cap to ~2000 chars so we don't blow up the textarea / context. Most
    // useful slice is the tail (errors, warnings tend to be at the end).
    const slice = text.length > 2000 ? '…\n' + text.slice(-2000) : text
    w.__appendToInput?.(`Here is the output:\n\`\`\`\n${slice}\n\`\`\``)
  }

  return (
    <div>
      {hiddenChars > 0 && (
        <div style="font-size:10px;color:var(--text-muted);padding:2px 2px 4px;font-style:italic">
          Output truncated — showing last {Math.round(renderText.length / 1000)}KB of {Math.round(text.length / 1000)}KB. Use Copy / Save for the full output.
        </div>
      )}
      <pre
        style={`margin:0;padding:8px 12px;background:var(--bg-sidebar);color:${isError ? 'var(--accent-red)' : 'var(--text-primary)'};font-family:var(--font-mono);font-size:11px;line-height:1.5;white-space:pre-wrap;word-break:break-word;max-height:${shouldFold ? '480px' : 'none'};overflow:auto;border-radius:var(--radius-xs);border:1px solid ${isError ? 'var(--tint-red-border)' : 'var(--bg-surface)'}`}
      >
        {segments.map((seg, i) => (
          <span key={i} style={seg.kind === 'stderr' ? 'color:var(--accent-red)' : undefined}>
            {seg.text}
          </span>
        ))}
      </pre>
      <div style="display:flex;gap:6px;margin-top:4px;align-items:center;flex-wrap:wrap">
        <BlockAction icon="fa-copy" label="Copy output" onClick={copyOutput} disabled={!text} />
        <BlockAction icon="fa-share-nodes" label="Share" onClick={shareAsMarkdown} disabled={!text} title="Copy as markdown for paste into chat / docs" />
        <BlockAction icon="fa-arrow-down-to-line" label="Use as input" onClick={useOutputAsInput} disabled={!text} title="Paste this output into the input bar (last 2000 chars)" />
        <span style="flex:1" />
        {totalLines > RESULT_PREVIEW_LINES && (
          <button
            onClick={() => setExpanded(!expanded)}
            type="button"
            style="background:transparent;border:1px solid var(--bg-overlay);color:var(--text-secondary);padding:4px 10px;border-radius:var(--radius-xs);cursor:pointer;font-size:10px"
          >
            {expanded ? 'Collapse' : `Show all ${totalLines} lines`}
          </button>
        )}
      </div>
    </div>
  )
}

/** Compact icon+label button for the block-action toolbar. Kept inline so
 *  the renderer file stays self-contained — no shared "Button" needed. */
function BlockAction({
  icon, label, onClick, disabled, title,
}: {
  icon: string; label: string; onClick: () => void; disabled?: boolean; title?: string
}): JSX.Element {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      title={title ?? label}
      style={`
        display:inline-flex;align-items:center;gap:5px;
        padding:4px 8px;border-radius:var(--radius-xs);
        background:transparent;border:1px solid var(--bg-surface);
        color:${disabled ? 'var(--bg-overlay)' : 'var(--text-secondary)'};
        font-size:10px;cursor:${disabled ? 'default' : 'pointer'};
        transition:background 0.12s,color 0.12s,border-color 0.12s;
      `}
      onMouseEnter={(e: any) => {
        if (disabled) return
        e.currentTarget.style.background = 'var(--bg-primary)'
        e.currentTarget.style.color = 'var(--text-primary)'
        e.currentTarget.style.borderColor = 'var(--bg-overlay)'
      }}
      onMouseLeave={(e: any) => {
        e.currentTarget.style.background = 'transparent'
        e.currentTarget.style.color = disabled ? 'var(--bg-overlay)' : 'var(--text-secondary)'
        e.currentTarget.style.borderColor = 'var(--bg-surface)'
      }}
    >
      <i class={`fas ${icon}`} style="font-size:10px" />
      {label}
    </button>
  )
}

interface Segment { kind: 'stdout' | 'stderr'; text: string }

function parseBashResult(text: string): Segment[] {
  const re = /<stderr>([\s\S]*?)<\/stderr>/g
  const out: Segment[] = []
  let lastIdx = 0
  let m: RegExpExecArray | null
  while ((m = re.exec(text)) !== null) {
    if (m.index > lastIdx) out.push({ kind: 'stdout', text: text.slice(lastIdx, m.index) })
    out.push({ kind: 'stderr', text: m[1] ?? '' })
    lastIdx = m.index + m[0].length
  }
  if (lastIdx < text.length) out.push({ kind: 'stdout', text: text.slice(lastIdx) })
  return out.length > 0 ? out : [{ kind: 'stdout', text }]
}
