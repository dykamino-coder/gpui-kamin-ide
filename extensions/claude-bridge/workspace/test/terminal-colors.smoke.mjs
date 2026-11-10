import './linux-browser-only.mjs'
import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'
import { test } from 'node:test'
import { chromium } from 'playwright'

const require = createRequire(new URL('../../webview/package.json', import.meta.url))
const { build } = require('esbuild')

// The actual xterm renderer and production color/query adapters. No second UI,
// model or native CEF claim: this is the bounded terminal primitive contract.
test('two browser terminal replicas apply colors and suppress color and size replies', { timeout: 30000 }, async t => {
  const bundle = await build({ write: false, bundle: true, format: 'iife', platform: 'browser',
    stdin: { resolveDir: fileURLToPath(new URL('../../webview/', import.meta.url)), contents: `
      import { Terminal } from '@xterm/xterm';
      import { attachTerminalColors } from './src/browser/terminal-colors';
      import { guardTerminalQueries } from './src/browser/terminal-query-guards';
      const terminal = new Terminal({cols:80, rows:24, allowProposedApi:true, windowOptions:{getWinSizeChars:true}});
      terminal.open(document.getElementById('terminal'));
      attachTerminalColors(terminal); guardTerminalQueries(terminal.parser);
      window.replies = []; terminal.onData(data => window.replies.push(data));
      window.terminal = terminal;
      window.write = data => new Promise(resolve => terminal.write(data, resolve));
    ` },
  })
  const server = createServer((request, response) => {
    response.setHeader('Content-Type', request.url === '/terminal.js' ? 'text/javascript' : 'text/html')
    response.end(request.url === '/terminal.js' ? bundle.outputFiles[0].contents : '<div id="terminal"></div><script src="/terminal.js"></script>')
  })
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
  t.after(() => new Promise(resolve => server.close(resolve)))
  const browser = await chromium.launch({ headless: true })
  t.after(() => browser.close())
  const pages = await Promise.all([browser.newPage(), browser.newPage()])
  await Promise.all(pages.map(async page => {
    await page.goto(`http://127.0.0.1:${server.address().port}`)
    await page.waitForFunction(() => !!window.write)
  }))
  for (const page of pages) {
    await page.evaluate(() => window.write('\x1b]10;?;?;?\x07\x1b]4;1;?;255;?\x07\x1b[6n\x1b[18t'))
    assert.deepEqual(await page.evaluate(() => window.replies), [], 'Renderer responses would become duplicate PTY input')
    await page.evaluate(async () => { window.terminal.resize(111, 37); await window.write('\x1b[18t') })
    assert.deepEqual(await page.evaluate(() => window.replies), [], 'Resized replicas must not report their page geometry')
    // Mixed mutations/queries must apply mutations without falling through to
    // xterm's own OSC handlers, which would send a response for each page.
    await page.evaluate(() => window.write('\x1b]4;1;#123456;1;?;200;#abc\x1b\\\x1b]10;#010203;#040506;?\x07'))
    const theme = await page.evaluate(() => window.terminal.options.theme)
    assert.equal(theme.red, '#123456')
    assert.equal(theme.extendedAnsi[184], '#aabbcc')
    assert.equal(theme.foreground, '#010203')
    assert.equal(theme.background, '#040506')
    assert.deepEqual(await page.evaluate(() => window.replies), [])
    await page.evaluate(() => window.write('\x1b]104\x07\x1b]110\x07\x1b]111\x07'))
    assert.equal(await page.evaluate(() => window.terminal.options.theme.red), '#f38ba8')
    assert.equal(await page.evaluate(() => window.terminal.options.theme.background), '#0b0c0f')
    await page.evaluate(() => { window.terminal.input('user-input', true) })
    assert.deepEqual(await page.evaluate(() => window.replies), ['user-input'], 'Real user input remains enabled')
  }
})
