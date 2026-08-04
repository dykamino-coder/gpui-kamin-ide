import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { FormSelect } from './FormSelect'
import { Field } from '../../ui/Field'
import { Button } from '../../ui/Button'
import { showToast } from '../../../signals/toasts'
import { HookTestRunner } from './HookTestRunner'

// Claude Code 2.1.198's real event set (verified against the CLI binary).
// Keep in sync with src/core/hooks/types.ts:CLI_HOOK_EVENTS — events the CLI
// never fires must not be offered here (they'd write dead settings keys).
const CLI_EVENTS = [
  'PreToolUse', 'PostToolUse', 'PostToolUseFailure', 'Notification',
  'UserPromptSubmit', 'SessionStart', 'SessionEnd', 'Stop',
  'SubagentStart', 'SubagentStop', 'PreCompact',
  'PermissionRequest', 'MessageDisplay', 'Elicitation',
  'TaskCompleted', 'TeammateIdle', 'Setup',
] as const
const BRIDGE_EVENTS = [
  'BridgeReconnect', 'MicAudioDetected', 'ConnectorStatusChanged',
  'AutoUpdateAvailable', 'PluginInstalled', 'PluginUninstalled', 'MarketplaceUpdated',
] as const
const ALL_EVENTS: readonly string[] = [...CLI_EVENTS, ...BRIDGE_EVENTS]
const BRIDGE_EVENTS_SET = new Set<string>(BRIDGE_EVENTS)

interface Props {
  initial?: { event: string; matcher?: string; handler: any; index?: { matcherIdx: number; handlerIdx: number } }
  onSave: () => void
  onCancel: () => void
}

/** Form for creating/editing a hook. Writes through hooks:save IPC. */
export function HookEditor({ initial, onSave, onCancel }: Props): JSX.Element {
  const bridge = useBridge()
  const [event, setEvent] = useState(initial?.event ?? 'PostToolUse')
  const [matcher, setMatcher] = useState(initial?.matcher ?? '')
  const [type, setType] = useState<string>(initial?.handler?.type ?? 'command')
  const [command, setCommand] = useState<string>(initial?.handler?.command ?? '')
  const [prompt, setPrompt] = useState<string>(initial?.handler?.prompt ?? '')
  const [url, setUrl] = useState<string>(initial?.handler?.url ?? '')
  const [shell, setShell] = useState<string>(initial?.handler?.shell ?? 'bash')
  const [host, setHost] = useState<string>(initial?.handler?.host ?? 'auto')
  const [timeout, setTimeoutSec] = useState<string>(String(initial?.handler?.timeout ?? 30))
  const [ifCond, setIfCond] = useState<string>(initial?.handler?.if ?? '')

  const [showTestRunner, setShowTestRunner] = useState(false)

  function buildHandler(): any {
    const t = Number(timeout) || 30
    const h: any = { type, timeout: t }
    if (ifCond) h.if = ifCond
    if (type === 'command') {
      h.command = command
      h.shell = shell
      h.host = host
    } else if (type === 'prompt') {
      h.prompt = prompt
    } else if (type === 'agent') {
      h.prompt = prompt
    } else if (type === 'http') {
      h.url = url
    }
    return h
  }

  async function handleSave(): Promise<void> {
    const draft = { event, matcher: matcher || undefined, handler: buildHandler(), index: initial?.index }
    const r = await bridge.hooksSave(draft as any)
    if (r?.ok) onSave()
    else showToast({ type: 'error', title: 'Save failed', message: r?.error ?? 'unknown', duration: 5000 })
  }

  function handleTest(): void {
    setShowTestRunner(true)
  }

  const fieldStyle = `
    width:100%;background:var(--bg-base);border:1px solid var(--bg-surface);
    border-radius:var(--radius-sm);color:var(--text-primary);
    padding:6px 10px;font-size:var(--fs-base);font-family:inherit;
  `
  // Field owns the label + column layout now; this wrapper only supplies the
  // inter-field vertical rhythm the old rowStyle used to.
  const rowStyle = `margin-bottom:var(--space-3)`

  return (
    <div style="
      position:fixed;inset:0;background:var(--overlay-modal);
      display:flex;align-items:center;justify-content:center;z-index:var(--z-modal);
    " onClick={onCancel}>
      <div
        onClick={(e: any) => e.stopPropagation()}
        style="
          background:var(--bg-mantle);border-radius:var(--radius-lg);
          padding:var(--space-5);min-width:560px;max-width:720px;max-height:85vh;
          overflow-y:auto;box-shadow:var(--shadow-lg);
        "
      >
        <h2 style="margin:0 0 var(--space-4);font-size:var(--fs-xl);color:var(--text-primary)">
          {initial?.index ? 'Edit hook' : 'New hook'}
        </h2>

        <div style={rowStyle}>
          <Field
            label="Event"
            hint={BRIDGE_EVENTS_SET.has(event)
              ? <><i class="fas fa-bridge" /> Bridge-emit — fires from open-claude-bridge, not from Claude CLI</>
              : <><i class="fas fa-bolt" /> Stock Claude Code event — fires from CLI</>}
          >
            <FormSelect
              value={event}
              onChange={setEvent}
              options={[
                ...CLI_EVENTS.map(ev => ({ value: ev, label: ev, icon: 'fa-bolt', description: 'Claude Code stock event' })),
                ...BRIDGE_EVENTS.map(ev => ({ value: ev, label: ev, icon: 'fa-bridge', description: 'Bridge-emit (custom over Claude Code)' })),
              ]}
            />
          </Field>
        </div>

        <div style={rowStyle}>
          <Field label="Matcher" hint={<>optional — e.g. <code>Bash</code> / <code>Edit|Write</code></>}>
            <input style={fieldStyle} value={matcher} onInput={(e: any) => setMatcher(e.target.value)} placeholder="leave empty to match all" />
          </Field>
        </div>

        <div style={rowStyle}>
          <Field label="Type">
          <div style="display:flex;gap:4px">
            {['command', 'prompt', 'agent', 'http'].map(t => (
              <button
                key={t}
                type="button"
                onClick={() => setType(t)}
                style={`
                  flex:1;padding:6px;border-radius:var(--radius-sm);
                  background:${type === t ? 'var(--accent-primary)' : 'var(--bg-base)'};
                  color:${type === t ? 'var(--bg-mantle)' : 'var(--text-primary)'};
                  border:1px solid var(--bg-surface);cursor:pointer;
                  font-size:var(--fs-sm);font-weight:600;
                `}
              >{t}</button>
            ))}
          </div>
          </Field>
        </div>

        {type === 'command' && (
          <>
            <div style={rowStyle}>
              <Field label="Command">
                <textarea
                  style={fieldStyle + ';min-height:80px;font-family:var(--font-mono)'}
                  value={command}
                  onInput={(e: any) => setCommand(e.target.value)}
                  placeholder="prettier --write $CLAUDE_FILE_PATHS"
                />
              </Field>
            </div>
            <div style="display:grid;grid-template-columns:1fr 1fr;gap:var(--space-3);margin-bottom:var(--space-3)">
              <Field label="Shell">
                <FormSelect
                  value={shell}
                  onChange={setShell}
                  options={[
                    { value: 'bash', label: 'bash', icon: 'fa-terminal' },
                    { value: 'powershell', label: 'powershell', icon: 'fa-square-binary' },
                    { value: 'sh', label: 'sh', icon: 'fa-terminal' },
                    { value: 'zsh', label: 'zsh', icon: 'fa-terminal' },
                  ]}
                />
              </Field>
              <Field label="Where runs">
                <FormSelect
                  value={host}
                  onChange={setHost}
                  options={[
                    { value: 'auto', label: 'auto (smart default)', icon: 'fa-shuffle', description: 'Bridge picks based on event/matcher' },
                    { value: 'local', label: 'local — your machine', icon: 'fa-bolt', description: 'Files, git, npm, IDE access' },
                    { value: 'server', label: 'server — bridge container', icon: 'fa-server', description: 'JSON transformation / HTTP only' },
                  ]}
                />
              </Field>
            </div>
          </>
        )}

        {(type === 'prompt' || type === 'agent') && (
          <div style={rowStyle}>
            <Field label="Prompt" hint={<>use <code>$ARGUMENTS</code> for hook input JSON</>}>
              <textarea
                style={fieldStyle + ';min-height:120px'}
                value={prompt}
                onInput={(e: any) => setPrompt(e.target.value)}
                placeholder="Verify that this Bash command is safe..."
              />
            </Field>
          </div>
        )}

        {type === 'http' && (
          <div style={rowStyle}>
            <Field label="URL">
              <input style={fieldStyle} value={url} onInput={(e: any) => setUrl(e.target.value)} placeholder="https://hooks.slack.com/..." />
            </Field>
          </div>
        )}

        <div style="display:grid;grid-template-columns:1fr 1fr;gap:var(--space-3);margin-bottom:var(--space-3)">
          <Field label="Timeout (sec)">
            <input style={fieldStyle} type="number" min="1" max="600" value={timeout} onInput={(e: any) => setTimeoutSec(e.target.value)} />
          </Field>
          <Field label="If condition" hint={<>permission rule, optional — e.g. <code>Bash(git *)</code></>}>
            <input style={fieldStyle} value={ifCond} onInput={(e: any) => setIfCond(e.target.value)} placeholder="Bash(git *)" />
          </Field>
        </div>

        <div style="display:flex;gap:var(--space-2);justify-content:flex-end;margin-top:var(--space-4)">
          <Button variant="ghost" onClick={onCancel}>Cancel</Button>
          <Button variant="secondary" icon="fa-flask" onClick={handleTest}>Test…</Button>
          <Button variant="primary" onClick={handleSave}>Save</Button>
        </div>
      </div>
      {showTestRunner && (
        <HookTestRunner
          asModal
          event={event}
          matcher={matcher || undefined}
          handler={buildHandler()}
          onClose={() => setShowTestRunner(false)}
        />
      )}
    </div>
  )
}
