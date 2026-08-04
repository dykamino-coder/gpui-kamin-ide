// Links in chat text must be clickable — INCLUDING a URL sealed in a backtick.
//
// THE BUG the user hit: the model writes action URLs as inline code
// (`http://localhost:3094/asset-unblock-app/`). linkify runs AFTER inline-code
// and deliberately skips code, so that URL rendered as dead <code> text — zero
// anchors, nothing to click. Markdown links and bare URLs already worked; only
// the backtick-sealed URL was mute.
//
// Run: from the kamin-ide repo root →  npx tsx <abs path to this file>

import { renderMarkdown } from './render-markdown.ts'

let failures = 0
function assert(cond: boolean, msg: string): void {
  if (!cond) { failures++; console.error('  ✗ ' + msg) }
}
const anchors = (html: string): number => (html.match(/data-ext-link/g) ?? []).length
const hrefs = (html: string): string[] => [...html.matchAll(/href="([^"]+)"/g)].map((m) => m[1])

console.log('Scenario 1: the exact case — a URL sealed in inline code is clickable')
{
  const html = renderMarkdown('Открывай `http://localhost:3094/asset-unblock-app/` → редирект')
  assert(anchors(html) === 1, `S1 one anchor (got ${String(anchors(html))})`)
  assert(hrefs(html)[0] === 'http://localhost:3094/asset-unblock-app/', `S1 href is the URL (got ${String(hrefs(html)[0])})`)
  assert(html.includes('data-ext-link'), 'S1 routed through the external-link handler')
  // Still looks like code — the mono pill styling survives.
  assert(/font-family:var\(--font-mono\)/.test(html), 'S1 keeps the mono code styling')
}

console.log('Scenario 2: the forms that already worked still work')
{
  assert(anchors(renderMarkdown('см http://localhost:3094 вот')) === 1, 'S2 bare URL')
  assert(anchors(renderMarkdown('[label](https://example.com/p)')) === 1, 'S2 markdown link')
  assert(anchors(renderMarkdown('www.example.com')) === 1, 'S2 www. bare')
  const md = renderMarkdown('[x](https://a.co)')
  assert(hrefs(md)[0] === 'https://a.co', 'S2 markdown href preserved')
}

console.log('Scenario 3: NON-url code stays literal code (no false anchors)')
{
  const html = renderMarkdown('run `npm install` first')
  assert(anchors(html) === 0, `S3 a normal code span is not linked (got ${String(anchors(html))})`)
  assert(html.includes('<code'), 'S3 still rendered as <code>')
}

console.log('Scenario 4: the URL-in-code anchor is not double-linked by linkify')
{
  const html = renderMarkdown('`https://example.com/a`')
  assert(anchors(html) === 1, `S4 exactly one anchor, not nested (got ${String(anchors(html))})`)
  assert(!/<a[^>]*><a/.test(html), 'S4 no nested <a><a>')
}

console.log('Scenario 5: a URL with query params keeps its ampersands usable')
{
  const html = renderMarkdown('`https://x.co/p?a=1&b=2`')
  assert(anchors(html) === 1, 'S5 linked')
  // In the HTML attribute it's &amp;; the DOM decodes it to & when read back.
  assert(/href="https:\/\/x\.co\/p\?a=1&(amp;)?b=2"/.test(html), `S5 ampersand preserved in href (got ${String(hrefs(html)[0])})`)
}

console.log('')
if (failures === 0) console.log('✅ ALL LINK-RENDER CHECKS PASSED')
else { console.error(`❌ ${failures} check(s) FAILED`); process.exit(1) }
