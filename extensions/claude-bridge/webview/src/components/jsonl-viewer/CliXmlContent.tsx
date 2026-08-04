import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { extractTag, extractTeammateMessages } from './utils'
import { CollapsibleBlock } from './CollapsibleBlock'

export function CliXmlContent({ text }: { text: string }): JSX.Element {
  const clean = text.replace(/\x1b\[[0-9;]*m/g, '')

  const cmdName = extractTag(clean, 'command-name')
  const cmdArgs = extractTag(clean, 'command-args')
  const stdout = extractTag(clean, 'local-command-stdout')
  const stderr = extractTag(clean, 'local-command-stderr')
  const caveat = extractTag(clean, 'local-command-caveat')
  const sysReminder = extractTag(clean, 'system-reminder')
  const errorDetails = extractTag(clean, 'error-details')
  const memContext = extractTag(clean, 'claude-mem-context')
  const bgInfo = extractTag(clean, 'claude_background_info')
  const fastMode = extractTag(clean, 'fast_mode_info')
  const envBlock = extractTag(clean, 'env')
  const envDetails = extractTag(clean, 'environment_details') || extractTag(clean, 'environment-details')
  const taskNotif = extractTag(clean, 'task-notification')
  const userMsg = extractTag(clean, 'user_message')
  const teammateMessages = extractTeammateMessages(clean)

  const TEAMMATE_COLORS: Record<string, string> = {
    blue: 'var(--accent-primary)', green: 'var(--accent-green)', red: 'var(--accent-red)', yellow: 'var(--accent-yellow)',
    purple: 'var(--accent-purple)', cyan: 'var(--accent-teal)', pink: 'var(--accent-pink)', orange: 'var(--accent-orange)',
  }

  const hasContent = cmdName || stdout || stderr || errorDetails || sysReminder ||
    (caveat && !cmdName) || memContext || bgInfo || fastMode || envBlock ||
    envDetails || taskNotif || userMsg || teammateMessages.length > 0

  if (!hasContent) {
    const stripped = clean.replace(/<\/?[a-z][\w-]*(?:\s[^>]*)?>|<\/teammate-message>/gi, '').trim()
    return <div class="entry-text cli-xml-content">{stripped || clean}</div>
  }

  return (
    <div class="entry-text cli-xml-content">
      {cmdName && (
        // flex-shrink:0 + nowrap на ИМЕНИ: без них длинные аргументы забирали
        // всю ширину строки, и `/goal` схлопывался в вертикальный столбик из
        // букв (жалоба юзера). Аргументы — отдельным блоком под именем: они
        // бывают в абзац длиной, в одну строку с именем не помещаются.
        <div style="margin-bottom:4px">
          <span style="color:var(--accent-green);font-family:'Cascadia Code','Fira Code',monospace;font-weight:600;white-space:nowrap;flex-shrink:0">
            {cmdName}
          </span>
          {cmdArgs && (
            <div style="color:var(--accent-primary);margin-top:3px;white-space:pre-wrap;overflow-wrap:anywhere;line-height:1.5">
              {cmdArgs}
            </div>
          )}
        </div>
      )}
      {/* Вывод команды дублирует аргументы (`/goal <текст>` → «Goal set: <тот
          же текст>») — печатаем только НЕ-дублирующую часть, иначе один и тот
          же абзац стоял в пузыре дважды. */}
      {stdout && !(cmdArgs && stdout.includes(cmdArgs.trim()) && cmdArgs.trim().length > 24) && (
        <div style="color:var(--text-secondary);font-size:12px;padding:4px 0;white-space:pre-wrap;overflow-wrap:anywhere">
          {stdout.replace(/\x1b\[[0-9;]*m/g, '')}
        </div>
      )}
      {stdout && cmdArgs && stdout.includes(cmdArgs.trim()) && cmdArgs.trim().length > 24 && (
        <div style="color:var(--text-muted-2);font-size:11px;padding:2px 0">
          {stdout.slice(0, stdout.indexOf(cmdArgs.trim())).replace(/\x1b\[[0-9;]*m/g, '').trim() || 'Готово'}
        </div>
      )}
      {stderr && (
        <div style="color:var(--accent-red);font-size:12px;padding:4px 0;">
          {stderr.replace(/\x1b\[[0-9;]*m/g, '')}
        </div>
      )}
      {errorDetails && (
        <div style="color:var(--accent-red);font-size:12px;padding:4px 0;border-left:2px solid var(--accent-red);padding-left:8px;">
          {errorDetails}
        </div>
      )}
      {sysReminder && <SysReminderBlock content={sysReminder} />}
      {caveat && !cmdName && (
        <div style="font-size:11px;color:var(--text-disabled);font-style:italic;">{caveat}</div>
      )}
      {memContext && <CollapsibleBlock label="memory-context" content={memContext} color="var(--accent-yellow)" />}
      {bgInfo && <CollapsibleBlock label="background-info" content={bgInfo} color="var(--accent-teal)" />}
      {fastMode && <CollapsibleBlock label="fast-mode-info" content={fastMode} color="var(--accent-pink)" />}
      {envBlock && <CollapsibleBlock label="environment" content={envBlock} color="var(--accent-primary)" />}
      {envDetails && <CollapsibleBlock label="environment-details" content={envDetails} color="var(--accent-primary)" />}
      {taskNotif && <CollapsibleBlock label="task-notification" content={taskNotif} color="var(--accent-green)" />}
      {userMsg && (
        <div style="font-size:13px;color:var(--text-primary);padding:4px 0;">{userMsg}</div>
      )}
      {teammateMessages.map((tm, i) => (
        <TeammateMessageCard
          key={i}
          id={tm.id}
          color={tm.color}
          body={tm.body}
          parsed={tm.parsed}
          accentColors={TEAMMATE_COLORS}
        />
      ))}
    </div>
  )
}

export function SysReminderBlock({ content }: { content: string }): JSX.Element {
  const [open, setOpen] = useState(false)
  const preview = content.slice(0, 500) + (content.length > 500 ? '...' : '')
  return (
    <div style="margin-top:4px;border-left:2px solid var(--bg-overlay);padding-left:8px;">
      <div
        style="font-size:11px;color:var(--text-muted);cursor:pointer;"
        onClick={() => setOpen(!open)}
      >
        {open ? '\u25BC' : '\u25B6'} system-reminder
      </div>
      {open && (
        <div style="font-size:11px;color:var(--text-disabled);white-space:pre-wrap;max-height:100px;overflow-y:auto;">
          {preview}
        </div>
      )}
    </div>
  )
}

interface TeammateMessageCardProps {
  id: string
  color?: string
  body: string
  parsed?: any
  accentColors: Record<string, string>
}

export function TeammateMessageCard({ id, color, body, parsed, accentColors }: TeammateMessageCardProps): JSX.Element {
  const accentColor = accentColors[color || ''] || 'var(--accent-primary)'

  const renderBody = (): JSX.Element => {
    if (!parsed) {
      const text = body.length > 500 ? body.substring(0, 500) + '...' : body
      return <div style="font-size:12px;color:var(--text-primary);white-space:pre-wrap;">{text}</div>
    }

    switch (parsed.type) {
      case 'teammate_terminated':
        return (
          <div style="font-size:12px;color:var(--text-muted);">
            <i class="fas fa-stop" style="color:var(--accent-yellow);font-size:10px" />{' '}
            {parsed.message || `${id} terminated`}
          </div>
        )
      case 'shutdown_approved':
      case 'shutdown_response': {
        const from = parsed.from || id
        return (
          <div style="font-size:12px;color:var(--accent-green);">
            <i class="fas fa-circle-check" style="color:var(--accent-green);font-size:10px" />{' '}
            <strong>{from}</strong> approved shutdown
          </div>
        )
      }
      case 'shutdown_request':
        return (
          <div style="font-size:12px;color:var(--accent-yellow);">
            <i class="fas fa-triangle-exclamation" style="color:var(--accent-yellow);font-size:10px" />{' '}
            Shutdown requested{parsed.reason ? ': ' + parsed.reason : ''}
          </div>
        )
      case 'idle_notification': {
        const from = parsed.from || id
        const reason = parsed.idleReason || 'idle'
        return (
          <div style="font-size:12px;color:var(--accent-teal);">
            <i class="fas fa-circle" style="color:var(--accent-teal);font-size:8px" />{' '}
            <strong>{from}</strong> — {reason}
            {parsed.summary && (
              <div style="margin-top:2px;color:var(--text-subtext);font-size:11px">{String(parsed.summary)}</div>
            )}
          </div>
        )
      }
      case 'message':
        return <div style="font-size:12px;color:var(--text-subtext);">{parsed.message || body}</div>
      default: {
        const text = parsed.message || body.substring(0, 200)
        return (
          <div style="font-size:12px;color:var(--text-subtext);">
            <span style="background:var(--bg-surface);padding:1px 6px;border-radius:var(--radius-sm);font-size:10px;color:var(--text-secondary)">
              {parsed.type}
            </span>{' '}
            {text}
          </div>
        )
      }
    }
  }

  const icon = id === 'system' ? 'fa-gear' : 'fa-user'
  const nameColor = id === 'system' ? 'var(--text-muted)' : accentColor

  return (
    <div style={`margin:4px 0;border-left:3px solid ${accentColor};padding:4px 8px;border-radius:var(--radius-xs);`}>
      <div style="display:flex;align-items:center;gap:6px;margin-bottom:2px;">
        <span style="font-size:12px"><i class={`fas ${icon}`} /></span>
        <span style={`font-size:12px;font-weight:600;color:${nameColor}`}>{id}</span>
      </div>
      {renderBody()}
    </div>
  )
}
