import { describe, it, expect } from 'vitest'
import { renderMarkdown } from './render-markdown'

/** Strip inline styles so assertions read as structure, not CSS. */
const html = (md: string): string => renderMarkdown(md).replace(/ style="[^"]*"/g, '')

describe('renderMarkdown — lists', () => {
  it('does not put <br> between list items', () => {
    const out = html('**Осталось**:\n- one\n- two\n- three')
    expect(out).not.toMatch(/<\/li>\s*<br\s*\/?>/i)
    // Whitespace between items is fine; a <br> is not.
    expect(out).toMatch(/<ul><li>one<\/li>\s*<li>two<\/li>\s*<li>three<\/li><\/ul>/)
  })

  it('keeps a list out of the paragraph that introduces it', () => {
    // The <ul> used to open inside one <p> and close inside the NEXT one.
    const out = html('**Осталось**:\n- one\n- two\n\nafter')
    expect(out).not.toMatch(/<p[^>]*>[^<]*<ul/i)
    expect(out).not.toMatch(/<\/ul>\s*<\/p>/i)
    expect(out.indexOf('<ul>')).toBeGreaterThan(-1)
    expect(out.indexOf('</ul>')).toBeGreaterThan(out.indexOf('<ul>'))
  })

  it('treats 3-space indented bullets as list items, not prose', () => {
    // Models routinely indent sub-bullets by three; the old pattern matched
    // exactly two, so these rendered as literal "   - text" joined by <br>.
    const out = html('1. Backend:\n   - alpha\n   - beta')
    expect(out).toContain('<li>alpha</li>')
    expect(out).toContain('<li>beta</li>')
    expect(out).not.toContain('   - alpha')
  })

  it('separates consecutive numbered blocks that carry nested bullets', () => {
    const out = html('1. First:\n   - a\n\n2. Second:\n   - b')
    // Each numbered line is its own paragraph, each nested list its own <ul>.
    expect(out.match(/<ul>/g)).toHaveLength(2)
    expect(out).toMatch(/<p>1\. First:<\/p>/)
    expect(out).toMatch(/<p>2\. Second:<\/p>/)
    expect(out).not.toMatch(/<br\s*\/?>\s*<ul>/i)
  })

  it('closes the list before the following paragraph', () => {
    const out = html('- one\n- two\n\ntail text')
    expect(out).toMatch(/<\/ul>\s*<p>tail text<\/p>/)
  })
})

describe('renderMarkdown — paragraphs', () => {
  it('keeps single newlines inside prose as hard breaks', () => {
    expect(html('line one\nline two')).toContain('line one<br>line two')
  })

  it('does not leave a <br> hanging before a block element', () => {
    expect(html('intro\n\n- item')).not.toMatch(/<br\s*\/?>\s*<ul/i)
  })
})

describe('renderMarkdown — attribute injection', () => {
  it('cannot break out of the href attribute via a quote in the URL', () => {
    // The URL patterns accept `"`, and the anchor interpolates the capture into
    // href="…" — so an unescaped quote injected an event handler into rendered
    // assistant text (which is fed to dangerouslySetInnerHTML).
    const out = renderMarkdown('[click](http://e/"onmouseover="alert(1))')
    // The payload may survive as inert TEXT inside the escaped value; what must
    // not happen is it becoming an attribute of the anchor.
    expect(out).not.toMatch(/<a[^>]*\sonmouseover\s*=/i)
    expect(out).toContain('&quot;onmouseover')
  })

  it('escapes quotes in a bare URL too', () => {
    const out = renderMarkdown('see http://e/"onfocus="alert(1) here')
    expect(out).not.toMatch(/<a[^>]*\sonfocus\s*=/i)
  })

  it('escapes quotes in plain prose', () => {
    expect(renderMarkdown('he said "hi"')).toContain('&quot;hi&quot;')
  })
})
