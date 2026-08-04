import hljs from 'highlight.js/lib/common'

// Bounded LRU cache so repeatedly re-rendering the same message text
// (Preact re-renders the JSONL viewer on every scroll/reflow) doesn't keep
// re-running highlight.js + all the regex passes. Keyed by the raw input;
// eviction is simple FIFO once we hit the limit — sufficient because the
// ordering we care about is "recently-rendered messages stay warm".
const RENDER_CACHE_LIMIT = 256
const renderCache = new Map<string, string>()

function cacheGet(key: string): string | undefined {
  const hit = renderCache.get(key)
  if (hit !== undefined) {
    // Move to the end (most-recently used) so it survives eviction longer.
    renderCache.delete(key)
    renderCache.set(key, hit)
  }
  return hit
}

function cacheSet(key: string, value: string): void {
  renderCache.set(key, value)
  if (renderCache.size > RENDER_CACHE_LIMIT) {
    // Evict the oldest entry (first inserted, per Map iteration order).
    const first = renderCache.keys().next().value
    if (first !== undefined) renderCache.delete(first)
  }
}

// Quotes are escaped too, not just the angle brackets. Later passes interpolate
// captured URLs into ATTRIBUTES (`href="${href}"` in extAnchor), and the URL
// patterns accept `"` — so `[x](http://e/"onmouseover="…)` closed the attribute
// and injected an event handler into the rendered anchor. This output is fed to
// `dangerouslySetInnerHTML` for assistant text, compact summaries and skill
// files, all of which can carry text the model merely echoed from elsewhere.
function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')
}

/** Belt-and-braces for a URL going into an attribute, in case it reached us
 *  without passing through `escapeHtml`. */
function escapeAttr(s: string): string {
  return s.replace(/"/g, '&quot;').replace(/'/g, '&#39;')
}

function highlightCode(code: string, lang: string): string {
  const trimmed = code.replace(/\n+$/, '')
  try {
    if (lang && hljs.getLanguage(lang)) {
      return hljs.highlight(trimmed, { language: lang, ignoreIllegals: true }).value
    }
    return hljs.highlightAuto(trimmed).value
  } catch {
    return escapeHtml(trimmed)
  }
}

// Trailing sentence punctuation shouldn't be part of a linked URL
// ("see http://x." → link http://x, not http://x.).
function stripUrlTrail(u: string): string {
  return u.replace(/[.,;:!?)\]]+$/, '')
}
// A code span whose ENTIRE content is one URL — nothing before or after. The
// backtick already delimits it exactly, so no trailing-punctuation trim is
// needed (that's for URLs sitting in prose). Content is HTML-escaped here, so an
// `&` reads as `&amp;`; `\S+` spans it fine.
const URL_ONLY_RE = /^(?:https?:\/\/|www\.)\S+$/i
// Anchor tagged `data-ext-link` so the chat's delegated click handler routes it
// to the host's openExternal (opens the OS browser) — a plain <a> can't navigate
// out of the sandboxed webview iframe. www.* is normalized to https://.
function extAnchor(url: string, label: string): string {
  const href = escapeAttr(/^www\./i.test(url) ? `https://${url}` : url)
  return `<a data-ext-link href="${href}" title="${href}" style="color:var(--accent-action);text-decoration:underline;cursor:pointer;">${label}</a>`
}
// Turn markdown links AND bare URLs into external anchors. Runs after HTML
// escaping + inline-code, so it operates on escaped text; the preceding-char
// guard (`[^"'>=/\]]`) keeps it from re-linking a URL already inside an
// href="…" attribute or a just-emitted anchor's label.
function linkify(text: string): string {
  text = text.replace(
    /\[([^\]]+)\]\((https?:\/\/[^)\s]+|mailto:[^)\s]+|www\.[^)\s]+)\)/g,
    (_m, label: string, url: string) => extAnchor(stripUrlTrail(url), label),
  )
  text = text.replace(
    /(^|[^"'>=/\]])((?:https?:\/\/|www\.)[^\s<>"'\])]+)/g,
    (_m, pre: string, url: string) => pre + extAnchor(stripUrlTrail(url), stripUrlTrail(url)),
  )
  return text
}

/** Simple markdown to HTML renderer for skill/agent content with syntax highlighting */
export function renderMarkdown(md: string): string {
  // Limit input size to prevent ReDoS and excessive rendering
  if (md.length > 100000) return '<p>Content too large to render</p>'

  const cached = cacheGet(md)
  if (cached !== undefined) return cached

  // Normalize line endings — mixed CRLF/CR causes the paragraph splitter to
  // miss blank lines, which in turn produced orphan <br> tags between headers.
  let text = md.replace(/\r\n?/g, '\n')

  // Collapse runs of 3+ blank lines to exactly one blank line so section
  // separators don't compound into giant gaps.
  text = text.replace(/\n{3,}/g, '\n\n')

  // Strip YAML frontmatter
  text = text.replace(/^---\s*\n[\s\S]*?\n---\s*\n?/, '')

  // Extract fenced code blocks FIRST (so we can highlight them before any
  // HTML escaping mangles the code). Replace with tokens we re-inject after
  // the rest of the rewrite passes.
  const codeBlocks: string[] = []
  text = text.replace(/```(\w*)\n?([\s\S]*?)```/g, (_m, lang: string, code: string) => {
    const highlighted = highlightCode(code, (lang || '').toLowerCase())
    // Header row: language label on the left, copy button on the right —
    // same baseline so the button visually aligns with "csharp" / "bash" /
    // etc. CSS in legacy-global.css owns the fade-in (.hljs-block:hover →
    // opacity:1), inline style only handles layout.
    const copyBtn = `<button type="button" class="hljs-copy-btn" aria-label="Copy code" data-tooltip="Copy" style="width:26px;height:22px;display:inline-flex;align-items:center;justify-content:center;background:var(--bg-surface);color:var(--text-secondary);border:1px solid var(--bg-overlay);border-radius:var(--radius-xs);cursor:pointer;font-size:11px;padding:0;flex:none"><i class="fas fa-copy" style="pointer-events:none"></i></button>`
    const headerRow = `<div class="hljs-header" style="display:flex;align-items:center;justify-content:space-between;gap:8px;padding:4px 8px 4px 12px;${lang ? 'border-bottom:1px solid var(--bg-surface);' : ''}min-height:22px"><span style="font-size:10px;color:var(--text-muted);font-family:monospace;text-transform:lowercase">${lang ? escapeHtml(lang) : ''}</span>${copyBtn}</div>`
    const html = `<pre class="hljs-block" style="position:relative;background:var(--bg-mantle);overflow-x:auto;font-size:12px;margin:8px 0;padding:0 0 10px;border-radius:var(--radius-sm);">${headerRow}<code class="hljs" style="display:block;background:var(--bg-mantle);color:var(--text-primary);font-family:ui-monospace,'Cascadia Code',Menlo,Consolas,monospace;line-height:1.5;padding:8px 16px 0">${highlighted}</code></pre>`
    codeBlocks.push(html)
    return `\x00CODEBLOCK${codeBlocks.length - 1}\x00`
  })

  // Escape HTML
  text = escapeHtml(text)

  const CODE_STYLE = 'font-family:var(--font-mono);font-size:13px;color:var(--accent-yellow);background:var(--bg-surface);padding:2px 10px;border-radius:var(--radius-xs);'
  text = text.replace(/`([^`]+)`/g, (_m, inner: string) => {
    // A URL sealed in a backtick — `http://localhost:3094/...` — is the ONE code
    // span worth making clickable: the model wraps action URLs this way, and the
    // linkify pass below deliberately skips code (a URL inside prose-code should
    // stay literal). Render it as an anchor that still LOOKS like code, so it
    // opens in the OS browser while keeping the mono pill. `inner` is already
    // HTML-escaped; a `&amp;` in the href decodes back to `&` when the click
    // handler reads getAttribute("href").
    if (URL_ONLY_RE.test(inner)) {
      const href = /^www\./i.test(inner) ? `https://${inner}` : inner
      return `<a data-ext-link href="${href}" title="${href}" style="${CODE_STYLE}text-decoration:underline;cursor:pointer;">${inner}</a>`
    }
    return `<code style="${CODE_STYLE}">${inner}</code>`
  })

  // Links: markdown [text](url) + bare URLs → external anchors (data-ext-link).
  // Runs after inline-code so a NON-URL code span stays literal; a URL-only span
  // was already turned into an anchor above, and linkify's preceding-char guard
  // keeps it from being re-linked.
  text = linkify(text)

  // Headers
  text = text.replace(/^#### (.+)$/gm, '<h4 style="font-size:13px;color:var(--text-primary);margin:8px 0 2px;">$1</h4>')
  text = text.replace(/^### (.+)$/gm, '<h3 style="font-size: var(--fs-md);color:var(--text-primary);margin:10px 0 4px;">$1</h3>')
  text = text.replace(/^## (.+)$/gm, '<h2 style="font-size:16px;color:var(--text-primary);margin:12px 0 5px;">$1</h2>')
  text = text.replace(/^# (.+)$/gm, '<h1 style="font-size: var(--fs-lg);color:var(--text-primary);margin:12px 0 6px;font-weight:700;">$1</h1>')

  // Bold and italic
  text = text.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
  text = text.replace(/\*(.+?)\*/g, '<em>$1</em>')

  // Unordered lists (with nesting). Items are tight by default. Nesting is 2+
  // spaces or a tab — models routinely indent sub-bullets by three, which the
  // old exactly-two-spaces pattern matched as neither nested NOR top-level, so
  // those lines fell through to the paragraph pass and rendered as literal
  // "   - text" joined by <br> (the "numbered blocks run together" report).
  text = text.replace(/^(?: {2,}|\t+)[*-] (.+)$/gm, '<li style="margin-left:20px;margin-top:2px;">$1</li>')
  text = text.replace(/^[*-] (.+)$/gm, '<li style="margin-top:2px;">$1</li>')

  // Wrap consecutive <li> in <ul>. Allow blank lines between items so
  // GitHub-style "loose" lists collapse into a single <ul> instead of
  // rendering each item as its own list with huge vertical gaps.
  // `.trim()` on the captured run matters: the trailing `\s*` otherwise pulls the
  // blank line that FOLLOWS the list inside the wrapper, which split `</ul>` into
  // a block of its own and left it orphaned inside an empty <p>.
  text = text.replace(/((?:<li[^>]*>.*<\/li>\s*)+)/g,
    (m) => `<ul style="margin:6px 0;padding-left:20px;color:var(--text-secondary);">${m.trim()}</ul>`)
  // Belt-and-suspenders: merge any adjacent <ul> blocks that slipped through.
  text = text.replace(/<\/ul>\s*<ul[^>]*>/g, '')
  // Put every list in a block of its OWN. The paragraph pass below leaves a
  // block alone only when it STARTS with a block-level tag, so a list sharing a
  // block with the line that introduces it ("**Остало́сь**:") was swallowed into
  // a <p>: every newline inside became a <br> — visible gaps between items —
  // and the <ul> opened in one <p> and closed in the NEXT one, which no browser
  // renders as written.
  text = text.replace(/[ \t]*\n?[ \t]*(<ul[^>]*>)/g, '\n\n$1').replace(/(<\/ul>)[ \t]*\n?[ \t]*/g, '$1\n\n')

  // Ordered-like numbered section headers (`1. Primary Request:`, `2. ...`).
  // We deliberately DON'T turn these into <ol> — in CLI summaries each
  // numbered line is a section header with interleaved prose, which makes
  // real <ol> restart at 1 for every block. Instead, force each numbered
  // line to be its own paragraph so the user-visible number is preserved
  // literally and the line break is not collapsed by the `\n` → space rule.
  text = text.replace(/\n(\d+\.\s+)/g, '\n\n$1')

  // Horizontal rules
  text = text.replace(/^---+$/gm, '<hr style="border:none;border-top:1px solid var(--bg-surface);margin:12px 0;">')

  // Tables (| col | col | format)
  text = text.replace(/((?:^\|.+\|$\n?)+)/gm, (tableBlock) => {
    const rows = tableBlock.trim().split('\n').filter(r => r.trim())
    if (rows.length < 2) return tableBlock
    // Check for separator row: every cell must be only dashes with
    // optional leading/trailing alignment colons. Splitting and checking
    // per-cell is more reliable than a single big regex against the raw
    // row, which over-matched (header rows survive `[\s:-]` cleanup).
    const sepIdx = rows.findIndex(r => {
      const cells = r.split('|').slice(1, -1).map(c => c.trim())
      return cells.length > 0 && cells.every(c => /^:?-+:?$/.test(c))
    })
    const headerRow = sepIdx > 0 ? rows[0] : null
    const dataRows = headerRow ? rows.filter((_, i) => i !== 0 && i !== sepIdx) : rows.filter((_, i) => i !== sepIdx)

    const parseRow = (row: string) => row.split('|').filter((_, i, a) => i > 0 && i < a.length - 1).map(c => c.trim())

    let html = '<table style="border-collapse:separate;border-spacing:0;width:max-content;margin:8px 0;font-size:12px;background:var(--bg-mantle);border-radius:var(--radius-sm);">'
    if (headerRow) {
      html += '<thead><tr>' + parseRow(headerRow).map(c =>
        `<th style="padding:10px 14px;background:var(--bg-surface);color:var(--text-primary);text-align:left;font-weight:600;border-bottom:1px solid var(--bg-overlay);white-space:nowrap;">${c}</th>`
      ).join('') + '</tr></thead>'
    }
    html += '<tbody>' + dataRows.map((row, i) =>
      `<tr style="background:${i % 2 ? 'color-mix(in srgb, var(--bg-surface) 35%, transparent)' : 'transparent'}">` + parseRow(row).map(c =>
        `<td style="padding:8px 14px;color:var(--text-secondary);border-top:1px solid color-mix(in srgb, var(--bg-surface) 50%, transparent);">${c}</td>`
      ).join('') + '</tr>'
    ).join('') + '</tbody></table>'
    return html
  })

  // Paragraphs — wrap non-tag lines. Anthropic returns logical lines (no
  // re-flow wrapping), so single newlines inside a paragraph are intentional
  // line breaks (dialog lines, list items written without `-`, etc.).
  // Render them as <br> — same as GFM `breaks: true` / Discord / Slack —
  // rather than CommonMark prose collapse-to-space which mangles dialog.
  // `  \n` (two trailing spaces) is also a hard break (no-op here since
  // we now treat all `\n` as breaks).
  text = text.split('\n\n').map(block => {
    const trimmed = block.trim()
    if (!trimmed) return ''
    // Pass through blocks that ALREADY start with a BLOCK-level tag (an earlier
    // pass emitted a heading/list/table/etc.) — those must not be re-wrapped in
    // <p>. But an inline tag (<strong>, <em>, <code>, <a>, <img>…) is NOT a block:
    // a paragraph like "**20. Ca.** …" is bolded to "<strong>20. Ca.</strong> …"
    // here, and the old `startsWith('<')` returned it unwrapped, so consecutive
    // \n\n items joined into one run-on line. Wrap those in <p> like any prose.
    if (/^<(?:ul|ol|li|h[1-6]|pre|table|thead|tbody|tr|td|th|hr|blockquote|div|p)\b/i.test(trimmed)) return trimmed
    if (/^\x00CODEBLOCK\d+\x00$/.test(trimmed)) return trimmed
    const withHardBreaks = trimmed.replace(/ {2,}\n/g, '<br>').replace(/\n/g, '<br>')
    return `<p style="margin:4px 0;line-height:1.55;">${withHardBreaks}</p>`
  }).join('\n')

  // Safety net: strip any orphan <br> that somehow landed between block
  // elements (e.g. <h2><br><br><h3> sequences from malformed input).
  // `li` belongs here too: a <br> between two items is the one that showed up as
  // blank lines inside a list.
  text = text.replace(/(<\/(?:h[1-6]|ul|ol|li|pre|table|p|hr)>)\s*(?:<br\s*\/?>\s*)+/gi, '$1')
  text = text.replace(/(?:<br\s*\/?>\s*)+(<(?:h[1-6]|ul|ol|li|pre|table|p|hr)\b)/gi, '$1')

  // Restore code blocks
  text = text.replace(/\x00CODEBLOCK(\d+)\x00/g, (_m, idx) => codeBlocks[Number(idx)] ?? '')

  cacheSet(md, text)
  return text
}
