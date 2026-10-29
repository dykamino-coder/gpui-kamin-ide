import { describe, expect, it } from 'vitest'
import { renderMarkdown } from './render-markdown'

const html = (md: string): string => renderMarkdown(md).replace(/ style="[^"]*"/g, '')

describe('renderMarkdown — blockquotes', () => {
  it('groups marked lines, blank paragraphs and nested markdown in one container', () => {
    const out = html('> **bold** and `code`\n> [link](https://example.com)\n>\n> - first\n> - *second*')
    expect(out).toMatch(/^<blockquote><p><strong>bold<\/strong> and <code>code<\/code><br><a /)
    expect(out).toContain('href="https://example.com"')
    expect(out).toMatch(/<ul><li>first<\/li>\s*<li><em>second<\/em><\/li><\/ul>\s*<\/blockquote>$/)
    expect(out.match(/<blockquote>/g)).toHaveLength(1)
    expect(out).not.toContain('&gt;')
  })

  it('interrupts prose and ends before an unmarked line without lazy continuation', () => {
    expect(html('before\n> quoted\nafter')).toBe('<p>before</p>\n<blockquote><p>quoted</p></blockquote>\n<p>after</p>')
  })

  it('accepts up to three spaces, optional marker spacing and nested quotes', () => {
    expect(html('   >outer\n > > inner')).toBe(
      '<blockquote><p>outer</p>\n<blockquote><p>inner</p></blockquote></blockquote>',
    )
    expect(html('> one\n\n> two').match(/<blockquote>/g)).toHaveLength(2)
    expect(html('    > indented')).not.toContain('<blockquote>')
  })

  it('escapes HTML and unsafe link attributes inside quotes', () => {
    const out = html(
      '> <script>alert("x")</script> & <img src=x onerror="bad">\n> [x](https://e/"onmouseover="bad)\n> \0CODEBLOCK0\0',
    )
    expect(out).toContain('&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt; &amp; &lt;img')
    expect(out).not.toMatch(/<(?:script|img)\b|<a[^>]*\sonmouseover\s*=/)
    expect(out).toContain('\uFFFDCODEBLOCK0\uFFFD')
  })

  it('keeps comparisons, inline code and escaped markers literal', () => {
    const out = html('a > b\n\\> escaped\n`> inline`\n&gt; entity')
    expect(out).not.toContain('<blockquote>')
    expect(out).toContain('a &gt; b')
    expect(out).toContain('<code>&gt; inline</code>')
    expect(out).toContain('&amp;gt; entity')
  })

  it('does not parse markers inside fenced code and renders quoted fenced code', () => {
    const out = html('```text\n> literal\n```\n\n> ```text\n> > quoted code\n> ```\n\n> prose')
    expect(out.match(/<blockquote>/g)).toHaveLength(2)
    expect(out.match(/<pre /g)).toHaveLength(2)
    expect(out).toContain('&gt; literal')
    expect(out).toContain('&gt; quoted code')
    expect(out).not.toContain('CODEBLOCK')
  })

  it('ignores quote markers inside longer, tilde and unfinished fences', () => {
    for (const md of ['````\n> literal\n```\n> still code\n````', '~~~\n> literal\n~~~', '```\n> literal']) {
      expect(html(md)).not.toContain('<blockquote>')
    }
  })

  it('bounds deeply nested untrusted quotes and keeps cache output stable', () => {
    const md = '> '.repeat(1000) + 'end'
    const out = html(md)
    expect(out.match(/<blockquote>/g)).toHaveLength(64)
    expect(out).toContain('end')
    expect(html(md)).toBe(out)
  })
})
