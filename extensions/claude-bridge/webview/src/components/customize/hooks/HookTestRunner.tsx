import type { JSX } from 'preact'
import { useState, useMemo } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'

interface Props {
  event: string
  matcher?: string
  handler: any
  /** When true, render as a standalone modal with overlay + close button.
   *  Otherwise render inline (used by HookEditor which already has a modal). */
  asModal?: boolean
  onClose?: () => void
}

interface TestResult {
  ok: boolean
  stdout: string
  stderr: string
  exitCode: number
  outcome: string
  durationMs: number
  effectiveHost?: string
  error?: string
}

/** CLI hook payload templates by event+matcher. These mirror the JSON
 *  Claude Code CLI pipes to hook commands on stdin. Plugins like redact
 *  branch on `tool_name` / `tool_input` / `tool_response` shape, so the
 *  template needs to match the actual event family — not a single
 *  one-size-fits-all blob. */
// Renderer doesn't expose Node's `process` (contextIsolation), so detect
// Windows via the user-agent string instead — accessing process.platform
// directly throws ReferenceError and corrupts the React/Preact render cycle.
const IS_WIN = typeof navigator !== 'undefined' && /Windows/i.test(navigator.userAgent)

function samplePayload(event: string, matcher?: string): Record<string, unknown> {
  const base = {
    session_id: 'test-session-' + Math.random().toString(36).slice(2, 8),
    transcript_path: '/tmp/transcript.jsonl',
    cwd: IS_WIN ? 'C:\\Users\\me\\proj' : '/home/me/proj',
    hook_event_name: event,
  }
  // Pick the first concrete tool from a matcher like "Read|Edit|Write".
  const firstTool = (matcher || '').split('|')[0].trim()

  switch (event) {
    case 'PreToolUse':
    case 'PostToolUse':
    case 'PostToolUseFailure': {
      const toolName = firstTool || 'Read'
      const tool_input = toolInputFor(toolName)
      const payload: Record<string, unknown> = { ...base, tool_name: toolName, tool_input }
      if (event !== 'PreToolUse') {
        payload.tool_response = toolResponseFor(toolName)
      }
      return payload
    }
    case 'UserPromptSubmit':
      return { ...base, prompt: 'Help me refactor this function' }
    case 'SessionStart':
      return { ...base, source: 'startup' }
    case 'SessionEnd':
    case 'Stop':
    case 'StopFailure':
      return { ...base, stop_hook_active: false }
    case 'SubagentStart':
    case 'SubagentStop':
      return { ...base, subagent_id: 'sub-1', stop_hook_active: false }
    case 'PreCompact':
    case 'PostCompact':
      return { ...base, trigger: 'manual', custom_instructions: '' }
    case 'Notification':
      return { ...base, message: 'Claude is waiting for your input' }
    default:
      return base
  }
}

function toolInputFor(name: string): Record<string, unknown> {
  switch (name) {
    case 'Read': return { file_path: IS_WIN ? 'C:\\Users\\me\\proj\\src\\app.ts' : '/home/me/proj/src/app.ts' }
    case 'Edit': return { file_path: '/home/me/proj/src/app.ts', old_string: 'const x = 1', new_string: 'const x = 2' }
    case 'Write': return { file_path: '/home/me/proj/src/new.ts', content: 'export const v = 1\n' }
    case 'Grep': return { pattern: 'TODO', glob: '*.ts', path: '/home/me/proj' }
    case 'Glob': return { pattern: '**/*.ts', path: '/home/me/proj' }
    case 'Bash': return { command: 'ls -la', description: 'list files' }
    default: return {}
  }
}

function toolResponseFor(name: string): Record<string, unknown> {
  switch (name) {
    case 'Read': return { content: 'export const v = 1\n', filePath: '/home/me/proj/src/app.ts' }
    case 'Bash': return { stdout: 'app.ts\nREADME.md\n', stderr: '', exitCode: 0 }
    case 'Grep': return { matches: ['src/app.ts:5: // TODO\n'], totalFiles: 1 }
    default: return { ok: true }
  }
}

/** Best-effort parse of hook stdout for the canonical decision shape:
 *    { "decision": "deny" | "allow" | "block", "message": "...", "reason": "..." }
 *  Some plugins also emit `{ permissionDecision, hookSpecificOutput, ... }`. */
function parseDecision(stdout: string): { decision?: string; message?: string; reason?: string; raw?: any } | null {
  if (!stdout.trim()) return null
  try {
    const parsed = JSON.parse(stdout)
    const decision = parsed?.decision || parsed?.permissionDecision || parsed?.hookSpecificOutput?.permissionDecision
    const message = parsed?.message || parsed?.hookSpecificOutput?.permissionDecisionReason
    const reason = parsed?.reason
    if (decision || message || reason) return { decision, message, reason, raw: parsed }
  } catch { /* not JSON — show raw */ }
  return null
}

export function HookTestRunner({ event, matcher, handler, asModal, onClose }: Props): JSX.Element {
  const bridge = useBridge()
  const [payloadText, setPayloadText] = useState(() => JSON.stringify(samplePayload(event, matcher), null, 2))
  const [running, setRunning] = useState(false)
  const [result, setResult] = useState<TestResult | null>(null)

  const decision = useMemo(() => result ? parseDecision(result.stdout) : null, [result])

  function regenSample(): void {
    setPayloadText(JSON.stringify(samplePayload(event, matcher), null, 2))
    setResult(null)
  }

  async function run(): Promise<void> {
    let parsed: Record<string, unknown>
    try {
      parsed = JSON.parse(payloadText)
    } catch (e: any) {
      setResult({ ok: false, stdout: '', stderr: '', exitCode: 0, outcome: 'error', durationMs: 0, error: 'Invalid JSON: ' + (e?.message ?? String(e)) })
      return
    }
    setRunning(true)
    setResult(null)
    try {
      const draft = { event, matcher: matcher || undefined, handler, mockPayload: parsed }
      const r = await bridge.hooksTest(draft as any)
      if (r?.ok) {
        const res = r.result ?? { stdout: '', stderr: '', exitCode: 0, outcome: 'unknown', durationMs: 0 }
        setResult({
          ok: true,
          stdout: res.stdout ?? '',
          stderr: res.stderr ?? '',
          exitCode: res.exitCode ?? 0,
          outcome: res.outcome ?? 'unknown',
          durationMs: res.durationMs ?? 0,
          effectiveHost: r.effectiveHost,
        })
      } else {
        setResult({ ok: false, stdout: '', stderr: '', exitCode: 0, outcome: 'error', durationMs: 0, error: r?.error ?? 'unknown' })
      }
    } finally {
      setRunning(false)
    }
  }

  const body = (
    <div style="display:flex;flex-direction:column;gap:var(--space-3)">
      <div style="display:flex;align-items:center;justify-content:space-between;gap:var(--space-3)">
        <div style="font-size:var(--fs-sm);color:var(--text-secondary)">
          <strong style="color:var(--text-primary)">{event}</strong>
          {matcher && <> · matcher <code style="background:var(--bg-base);padding:1px 6px;border-radius:var(--radius-xs);font-family:var(--font-mono);font-size:var(--fs-xs)">{matcher}</code></>}
        </div>
        <button
          type="button"
          onClick={regenSample}
          style="padding:4px 10px;background:transparent;border:1px solid var(--bg-surface);border-radius:var(--radius-sm);color:var(--text-secondary);cursor:pointer;font-size:var(--fs-xs)"
        >
          <i class="fas fa-rotate" /> Regenerate sample
        </button>
      </div>

      <div style="display:flex;flex-direction:column;gap:4px">
        <label style="font-size:var(--fs-sm);color:var(--text-secondary);font-weight:600">
          Stdin payload (JSON, fed to the hook on stdin)
        </label>
        <textarea
          value={payloadText}
          onInput={(e: any) => setPayloadText(e.target.value)}
          spellcheck={false}
          style="
            width:100%;min-height:220px;background:var(--bg-base);
            border:1px solid var(--bg-surface);border-radius:var(--radius-sm);
            color:var(--text-primary);padding:var(--space-2) var(--space-3);
            font-family:var(--font-mono);font-size:var(--fs-sm);resize:vertical;
          "
        />
        <div style="font-size:var(--fs-xs);color:var(--text-muted)">
          Edit any field — e.g. for redact's bash-guard set <code>tool_name: "Bash"</code> + <code>tool_input.command</code>.
        </div>
      </div>

      <div style="display:flex;justify-content:flex-end;gap:var(--space-2)">
        <button
          type="button"
          onClick={run}
          disabled={running}
          style="padding:8px 16px;background:var(--accent-yellow);border:none;border-radius:var(--radius-sm);color:var(--bg-mantle);cursor:pointer;font-weight:600;opacity:1"
        >
          {running ? <><i class="fas fa-spinner fa-spin" /> Running…</> : <><i class="fas fa-play" /> Run hook</>}
        </button>
      </div>

      {result && (
        <div style="display:flex;flex-direction:column;gap:var(--space-2);background:var(--bg-base);border-radius:var(--radius-sm);padding:var(--space-3)">
          <div style="display:flex;align-items:center;gap:var(--space-3);font-size:var(--fs-sm);font-family:var(--font-mono)">
            <strong style={`color:${result.outcome === 'success' ? 'var(--accent-green)' : 'var(--accent-red)'}`}>
              {result.outcome}
            </strong>
            <span>exit {result.exitCode}</span>
            <span>{result.durationMs}ms</span>
            {result.effectiveHost && <span>host: {result.effectiveHost}</span>}
          </div>

          {decision && (
            <div style={`
              padding:var(--space-2) var(--space-3);border-radius:var(--radius-sm);
              background:${decision.decision === 'deny' || decision.decision === 'block'
                ? 'var(--tint-red-medium)'
                : 'var(--tint-green-medium)'};
              border:1px solid ${decision.decision === 'deny' || decision.decision === 'block'
                ? 'var(--tint-red-border-strong)'
                : 'var(--tint-green-border)'};
            `}>
              <div style="display:flex;align-items:center;gap:6px;font-size:var(--fs-sm);font-weight:600;margin-bottom:4px">
                <i class={`fas ${decision.decision === 'deny' || decision.decision === 'block' ? 'fa-ban' : 'fa-check'}`}
                   style={`color:${decision.decision === 'deny' || decision.decision === 'block' ? 'var(--accent-red)' : 'var(--accent-green)'}`} />
                <span style="text-transform:uppercase;letter-spacing:0.04em">decision: {decision.decision || 'parsed'}</span>
              </div>
              {decision.message && <div style="font-size:var(--fs-sm);color:var(--text-primary);white-space:pre-wrap">{decision.message}</div>}
              {decision.reason && <div style="font-size:var(--fs-sm);color:var(--text-secondary);white-space:pre-wrap;margin-top:4px">Reason: {decision.reason}</div>}
            </div>
          )}

          {result.outcome === 'timeout' && (
            <div style="font-size:var(--fs-xs);color:var(--accent-yellow);background:var(--tint-yellow-medium);border:1px solid var(--tint-yellow-border);padding:var(--space-2) var(--space-3);border-radius:var(--radius-sm);line-height:1.5">
              <i class="fas fa-info-circle" style="margin-right:6px" />
              The hook ran for {result.durationMs}ms without exiting. Most common cause:
              the script reads from stdin but the payload below either wasn't consumed
              or the script is waiting on something else (network, subprocess).
              {!result.stdout && ' No bytes were written to stdout before kill.'}
            </div>
          )}

          <details style="font-size:var(--fs-xs);color:var(--text-secondary)">
            <summary style="cursor:pointer;font-weight:600;color:var(--text-secondary)">stdin sent ({payloadText.length} chars)</summary>
            <pre style="margin:4px 0 0;font-family:var(--font-mono);font-size:var(--fs-xs);background:var(--bg-mantle);padding:var(--space-2);border-radius:var(--radius-xs);white-space:pre-wrap;color:var(--text-primary)">{payloadText}</pre>
          </details>

          <details open={!decision && !!result.stdout} style="font-size:var(--fs-xs);color:var(--text-secondary)">
            <summary style="cursor:pointer;font-weight:600;color:var(--text-secondary)">stdout ({result.stdout?.length ?? 0} chars)</summary>
            <pre style="margin:4px 0 0;font-family:var(--font-mono);font-size:var(--fs-xs);background:var(--bg-mantle);padding:var(--space-2);border-radius:var(--radius-xs);white-space:pre-wrap;color:var(--text-primary)">{result.stdout || '(empty — the hook did not write to stdout)'}</pre>
          </details>
          {result.stderr && (
            <details open style="font-size:var(--fs-xs);color:var(--accent-red)">
              <summary style="cursor:pointer;font-weight:600">stderr ({result.stderr.length} chars)</summary>
              <pre style="margin:4px 0 0;font-family:var(--font-mono);font-size:var(--fs-xs);background:var(--bg-mantle);padding:var(--space-2);border-radius:var(--radius-xs);white-space:pre-wrap;color:var(--accent-red)">{result.stderr}</pre>
            </details>
          )}
          {result.error && (
            <div style="font-size:var(--fs-sm);color:var(--accent-red)">{result.error}</div>
          )}
        </div>
      )}
    </div>
  )

  if (!asModal) return body

  return (
    <div
      onClick={onClose}
      style="position:fixed;inset:0;background:var(--overlay-modal);display:flex;align-items:center;justify-content:center;z-index:var(--z-modal)"
    >
      <div
        onClick={(e: any) => e.stopPropagation()}
        style="background:var(--bg-mantle);border-radius:var(--radius-lg);padding:var(--space-5);min-width:600px;max-width:780px;max-height:88vh;overflow-y:auto;box-shadow:var(--shadow-lg)"
      >
        <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:var(--space-4)">
          <h2 style="margin:0;font-size:var(--fs-xl);color:var(--text-primary)">Test hook</h2>
          <button
            type="button"
            onClick={onClose}
            style="background:transparent;border:none;color:var(--text-muted);cursor:pointer;font-size:var(--fs-lg);padding:4px 8px"
            data-tooltip="Close"
          >
            <i class="fas fa-times" />
          </button>
        </div>
        {body}
      </div>
    </div>
  )
}
