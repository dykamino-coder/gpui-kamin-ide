import assert from 'node:assert/strict'
import { test } from 'node:test'
import headless from '@xterm/headless'
import { TerminalScreen } from '../src/terminal-screen.js'

test('host parser retains scrollback, alternate screen and input modes with zero pages', async t => {
  const responses: string[] = []
  const screen = new TerminalScreen(80, 24, () => assert.fail('Unexpected parser overflow'), data => responses.push(data))
  const restored = new headless.Terminal({ cols: 80, rows: 24, allowProposedApi: true })
  t.after(() => { screen.dispose(); restored.dispose() })
  screen.write('host-history\r\n\x1b[?1049h\x1b[H\x1b[?1002;1006happ-screen\x1b[6n', () => {})
  await screen.order(() => {})
  assert.ok(responses.some(data => data.endsWith('R')), 'The host answers DSR with no browser')
  const snapshot = screen.snapshot()
  await new Promise<void>(resolve => restored.write(snapshot.screen, resolve))
  assert.equal(restored.buffer.active.type, 'alternate')
  assert.equal(restored.modes.mouseTrackingMode, 'drag')
  assert.match(restored.buffer.active.getLine(0)!.translateToString(true), /app-screen/)
  assert.match(restored.buffer.normal.getLine(0)!.translateToString(true), /host-history/)
  assert.equal(snapshot.truncated, false)
})

test('host parser bounds pending input and preserves its last committed screen', async t => {
  let overflows = 0
  const screen = new TerminalScreen(80, 24, () => { overflows++ })
  t.after(() => screen.dispose())
  screen.write('committed', () => {})
  await screen.order(() => {})
  screen.write('x'.repeat(2 * 1024 * 1024 + 1), () => assert.fail('Overflowed output was committed'))
  screen.write('discarded', () => assert.fail('Output after overflow was committed'))
  await screen.order(() => {})
  assert.equal(overflows, 1)
  assert.match(screen.snapshot().screen, /committed/)
  assert.doesNotMatch(screen.snapshot().screen, /discarded/)
})

test('large styled history fits the websocket JSON budget and reports omitted older history', async t => {
  const screen = new TerminalScreen(200, 24, () => assert.fail('Unexpected parser overflow'))
  t.after(() => screen.dispose())
  const batch = (('\x1b[31mx\x1b[32my').repeat(90) + '\r\n').repeat(200)
  for (let i = 0; i < 28; i++) {
    screen.write(batch, () => {})
    await screen.order(() => {})
  }
  screen.write('\x1b[0mnewest-marker', () => {})
  await screen.order(() => {})
  const snapshot = screen.snapshot()
  assert.equal(snapshot.truncated, true)
  assert.ok(Buffer.byteLength(JSON.stringify(snapshot.screen)) <= 8 * 1024 * 1024)
  assert.match(snapshot.screen, /newest-marker/)
})
