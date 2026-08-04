import type { JSX, ComponentChildren } from 'preact'

// Render an agent description that follows Claude's "when to use" convention:
// free prose + zero or more <example>…</example> blocks, each containing
// <commentary>…</commentary> notes and inline `user:` / `assistant:` speaker
// lines. We don't run a real XML parser — just split on the known tag pairs so
// the output is readable at a glance without rendering raw `<example>` text.

interface Block {
  kind: 'prose' | 'example'
  text: string
}

function splitBlocks(raw: string): Block[] {
  const out: Block[] = []
  const re = /<example>([\s\S]*?)<\/example>/gi
  let last = 0
  let m: RegExpExecArray | null
  while ((m = re.exec(raw))) {
    if (m.index > last) {
      const prose = raw.slice(last, m.index).trim()
      if (prose) out.push({ kind: 'prose', text: prose })
    }
    out.push({ kind: 'example', text: m[1].trim() })
    last = m.index + m[0].length
  }
  if (last < raw.length) {
    const tail = raw.slice(last).trim()
    if (tail) out.push({ kind: 'prose', text: tail })
  }
  return out
}

interface ExamplePart {
  kind: 'text' | 'commentary' | 'user' | 'assistant'
  text: string
}

function splitExample(raw: string): ExamplePart[] {
  // First pull out <commentary>…</commentary> blocks.
  const parts: ExamplePart[] = []
  const commentaryRe = /<commentary>([\s\S]*?)<\/commentary>/gi
  let last = 0
  let m: RegExpExecArray | null
  while ((m = commentaryRe.exec(raw))) {
    if (m.index > last) {
      const chunk = raw.slice(last, m.index).trim()
      if (chunk) parts.push(...splitSpeakers(chunk))
    }
    parts.push({ kind: 'commentary', text: m[1].trim() })
    last = m.index + m[0].length
  }
  if (last < raw.length) {
    const chunk = raw.slice(last).trim()
    if (chunk) parts.push(...splitSpeakers(chunk))
  }
  return parts
}

// Split a chunk into speaker-tagged parts. Speakers appear as `user: …` or
// `assistant: …` at the start of a line. Everything before the first speaker
// is kept as plain text (typically a `Context:` line).
function splitSpeakers(chunk: string): ExamplePart[] {
  const lines = chunk.split(/\r?\n/)
  const result: ExamplePart[] = []
  let currentKind: ExamplePart['kind'] = 'text'
  let buffer: string[] = []

  const flush = () => {
    const joined = buffer.join('\n').trim()
    if (joined) result.push({ kind: currentKind, text: joined })
    buffer = []
  }

  for (const line of lines) {
    const speakerMatch = line.match(/^(user|assistant)\s*:\s*(.*)$/i)
    if (speakerMatch) {
      flush()
      currentKind = speakerMatch[1].toLowerCase() as 'user' | 'assistant'
      if (speakerMatch[2]) buffer.push(speakerMatch[2])
    } else {
      buffer.push(line)
    }
  }
  flush()
  return result
}

function Prose({ children }: { children: ComponentChildren }): JSX.Element {
  return <div style="color:var(--text-primary);font-size:13px;line-height:1.55;white-space:pre-wrap;word-break:break-word">{children}</div>
}

function SpeakerBlock({ who, text }: { who: 'user' | 'assistant'; text: string }): JSX.Element {
  const color = who === 'user' ? 'var(--accent-primary)' : 'var(--accent-green)'
  return (
    <div style="margin:6px 0;padding:8px 10px;background:var(--bg-sidebar);border-radius:var(--radius-xs)">
      <div style={`font-size:10px;font-weight:700;text-transform:uppercase;letter-spacing:0.6px;color:${color};margin-bottom:4px`}>{who}</div>
      <div style="color:var(--text-primary);font-size:12px;line-height:1.5;white-space:pre-wrap;word-break:break-word">{text}</div>
    </div>
  )
}

function Commentary({ text }: { text: string }): JSX.Element {
  return (
    <div style="margin:6px 0;padding:8px 10px;border:1px solid var(--bg-overlay);border-radius:var(--radius-xs);background:var(--bg-tint-primary)">
      <div style="font-size:10px;font-weight:700;text-transform:uppercase;letter-spacing:0.6px;color:var(--accent-yellow);margin-bottom:4px">Commentary</div>
      <div style="color:var(--text-subtext);font-size:12px;line-height:1.5;font-style:italic;white-space:pre-wrap;word-break:break-word">{text}</div>
    </div>
  )
}

function ExampleBlock({ text }: { text: string }): JSX.Element {
  const parts = splitExample(text)
  return (
    <div style="margin:8px 0;padding:10px 12px;border:1px solid var(--bg-surface);border-radius:var(--radius-sm);background:var(--bg-mantle)">
      <div style="font-size:10px;font-weight:700;text-transform:uppercase;letter-spacing:0.6px;color:var(--accent-purple);margin-bottom:6px">Example</div>
      {parts.map((p, i) => {
        if (p.kind === 'commentary') return <Commentary key={i} text={p.text} />
        if (p.kind === 'user' || p.kind === 'assistant') return <SpeakerBlock key={i} who={p.kind} text={p.text} />
        return (
          <div key={i} style="color:var(--text-primary);font-size:12px;line-height:1.5;white-space:pre-wrap;word-break:break-word;margin:4px 0">
            {p.text}
          </div>
        )
      })}
    </div>
  )
}

interface Props {
  text: string
}

export function AgentDescription({ text }: Props): JSX.Element {
  if (!text) return <div style="color:var(--text-muted);font-size:12px">No description.</div>
  const blocks = splitBlocks(text)
  return (
    <div>
      {blocks.map((b, i) =>
        b.kind === 'example'
          ? <ExampleBlock key={i} text={b.text} />
          : <Prose key={i}>{b.text}</Prose>,
      )}
    </div>
  )
}
