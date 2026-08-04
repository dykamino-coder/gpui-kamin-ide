import type { JSX } from 'preact'

interface EditInput {
  file_path?: string
  old_string?: string
  new_string?: string
  replace_all?: boolean
}

/** Compact two-column diff for Edit / MultiEdit: red lines show removed
 *  text, green lines show the replacement. Not a real line-by-line diff
 *  algorithm — CLI's FileEditTool stores old_string / new_string verbatim,
 *  so we just colour them whole. Keeps the payload readable at a glance
 *  without pulling in a diff-library dependency. */
export function EditToolRender({ input }: { input: EditInput | { edits?: EditInput[]; file_path?: string } }): JSX.Element {
  const isMulti = Array.isArray((input as any).edits)
  const filePath = (input as any).file_path as string | undefined
  const edits: EditInput[] = isMulti
    ? ((input as any).edits as EditInput[])
    : [input as EditInput]

  return (
    <div style="padding:6px 0">
      {filePath && (
        <div style="font-family:var(--font-mono);font-size:11px;color:var(--accent-primary);padding:4px 10px;background:var(--bg-primary);border:1px solid var(--bg-surface);border-radius:var(--radius-xs) var(--radius-xs) 0 0;border-bottom:none">
          <i class="fas fa-pen" style="margin-right:6px;color:var(--accent-yellow);font-size:10px" />
          {filePath}
          {isMulti && <span style="margin-left:8px;color:var(--text-muted)">· {edits.length} edits</span>}
        </div>
      )}
      <div style="border:1px solid var(--bg-surface);border-radius:0 0 var(--radius-xs) var(--radius-xs);overflow:hidden">
        {edits.map((e, i) => (
          <div key={i} style={i > 0 ? 'border-top:1px solid var(--bg-surface)' : ''}>
            {e.old_string !== undefined && e.old_string !== '' && (
              <DiffBlock sign="-" text={e.old_string} bg="var(--tint-red-soft-2)" color="var(--accent-red)" />
            )}
            {e.new_string !== undefined && (
              <DiffBlock sign="+" text={e.new_string} bg="var(--tint-green-soft)" color="var(--accent-green)" />
            )}
            {e.replace_all && (
              <div style="padding:2px 10px;font-size:10px;color:var(--accent-yellow);background:var(--bg-primary)">
                <i class="fas fa-repeat" style="margin-right:4px" />
                replace all occurrences
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  )
}

function DiffBlock({ sign, text, bg, color }: { sign: string; text: string; bg: string; color: string }): JSX.Element {
  // Patch-local line numbers: CLI's Edit tool records `old_string` /
  // `new_string` verbatim without a file-relative offset, so we can't
  // show absolute file lines. 1-based patch-local numbers give the user
  // something to point at ("look at line 4 of the removed block") —
  // `tabular-nums` + fixed width keeps the gutter visually stable even
  // when N goes past 9.
  const lines = text.split('\n')
  const gutterWidth = String(lines.length).length
  return (
    <pre style={`margin:0;padding:6px 0;background:${bg};color:${color};font-family:var(--font-mono);font-size:11px;line-height:1.5;white-space:pre-wrap;word-break:break-word;border-left:3px solid ${color}`}>
      {lines.map((line, i) => (
        <div key={i} style="display:flex;gap:0;padding:0 10px 0 0">
          <span style={`display:inline-block;min-width:${gutterWidth + 1}ch;text-align:right;margin-right:10px;padding-left:10px;opacity:0.45;user-select:none;font-variant-numeric:tabular-nums;border-right:1px solid var(--divider-soft)`}>{i + 1}</span>
          <span style="opacity:0.55;user-select:none;margin-right:8px;margin-left:8px">{sign}</span>
          <span style="flex:1;min-width:0">{line || ' '}</span>
        </div>
      ))}
    </pre>
  )
}
