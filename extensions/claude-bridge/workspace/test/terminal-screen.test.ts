import assert from 'node:assert/strict'
import { test } from 'node:test'
import headless from '@xterm/headless'
import { TerminalScreen } from '../src/terminal-screen.js'

test('host reports current text-area dimensions and keeps unrelated window operations disabled', async t => {
  const replies: string[] = []
  const screen = new TerminalScreen(80, 24, () => assert.fail('Unexpected parser overflow'), data => replies.push(data))
  t.after(() => screen.dispose())
  const write = async (value: string) => { screen.write(value, () => {}); await screen.order(() => {}) }
  await write('\x1b[18t')
  assert.deepEqual(replies.splice(0), ['\x1b[8;24;80t'])
  await screen.order(() => screen.resize(111, 37))
  await write('\x1b[18')
  assert.deepEqual(replies, [], 'Incomplete queries cannot produce replies')
  await write('t\x1b[18t')
  assert.deepEqual(replies.splice(0), ['\x1b[8;37;111t', '\x1b[8;37;111t'])
  // No pixel geometry exists in the host. Terminal output must not resize it.
  await write('\x1b[14t\x1b[16t\x1b[8;2;3t\x1b[18t')
  assert.deepEqual(replies.splice(0), ['\x1b[8;37;111t'])
  await screen.order(() => screen.resize(40, 10))
  await write('\x1b[?1049h\x1b[18t\x1b[?1049l\x1b[18t')
  assert.deepEqual(replies, ['\x1b[8;10;40t', '\x1b[8;10;40t'])
  assert.doesNotMatch(screen.snapshot().screen, /\x1b\[18t/, 'Replay must not repeat queries')
})

test('host answers color queries with no pages and retains mutations across screen sync', async t => {
  const replies: string[] = []
  const screen = new TerminalScreen(80, 24, () => assert.fail('Unexpected parser overflow'), data => replies.push(data))
  const replicaReplies: string[] = []
  const replica = new TerminalScreen(80, 24, () => assert.fail('Unexpected parser overflow'), data => replicaReplies.push(data))
  t.after(() => { screen.dispose(); replica.dispose() })
  const write = async (value: string) => { screen.write(value, () => {}); await screen.order(() => {}) }
  await write('\x1b]10;?\x07\x1b]11;?\x1b\\\x1b]4;1;?;16;?;255;?\x07')
  assert.deepEqual(replies.splice(0), [
    '\x1b]10;rgb:cfcf/d4d4/e2e2\x1b\\', '\x1b]11;rgb:0b0b/0c0c/0f0f\x1b\\',
    '\x1b]4;1;rgb:f3f3/8b8b/a8a8\x1b\\', '\x1b]4;16;rgb:0000/0000/0000\x1b\\',
    '\x1b]4;255;rgb:eeee/eeee/eeee\x1b\\',
  ])
  // Set/query pairs in one OSC and a sequence split across writes are ordered.
  await write('\x1b]4;1;#123456;1;?\x07\x1b]10;rgb:f/0/a;#010203;?\x1b\\\x1b]11;')
  await write('?\x07')
  assert.deepEqual(replies.splice(0), [
    '\x1b]4;1;rgb:1212/3434/5656\x1b\\', '\x1b]12;rgb:f5f5/e0e0/dcdc\x1b\\',
    '\x1b]11;rgb:0101/0202/0303\x1b\\',
  ])
  screen.write('colored-screen', () => {}); await screen.order(() => {})
  replica.write(screen.snapshot().screen, () => {}); await replica.order(() => {})
  replica.write('\x1b]4;1;?\x07\x1b]10;?;?\x07', () => {}); await replica.order(() => {})
  assert.deepEqual(replicaReplies, [
    '\x1b]4;1;rgb:1212/3434/5656\x1b\\', '\x1b]10;rgb:ffff/0000/aaaa\x1b\\',
    '\x1b]11;rgb:0101/0202/0303\x1b\\',
  ])
  assert.match(replica.snapshot().screen, /colored-screen/)
  await write('\x1b]104;1\x07\x1b]110\x07\x1b]111\x07\x1b]4;1;?\x07\x1b]10;?\x07')
  assert.deepEqual(replies, ['\x1b]4;1;rgb:f3f3/8b8b/a8a8\x1b\\', '\x1b]10;rgb:cfcf/d4d4/e2e2\x1b\\'])
})

test('invalid palette operations are inert and full reset restores default query values', async t => {
  const replies: string[] = []
  const screen = new TerminalScreen(80, 24, () => assert.fail('Unexpected parser overflow'), data => replies.push(data))
  t.after(() => screen.dispose())
  const write = async (value: string) => { screen.write(value, () => {}); await screen.order(() => {}) }
  await write('\x1b]4;256;#123456;-1;#123456;;#123456;1;invalid;2;#ffff\x07\x1b]11;#ffff\x07')
  await write('\x1b]4;1;?;2;?\x07\x1b]11;?\x07')
  assert.deepEqual(replies.splice(0), [
    '\x1b]4;1;rgb:f3f3/8b8b/a8a8\x1b\\', '\x1b]4;2;rgb:a6a6/e3e3/a1a1\x1b\\', '\x1b]11;rgb:0b0b/0c0c/0f0f\x1b\\',
  ])
  for (const reset of ['\x1b]104\x07\x1b]112\x07', '\x1bc']) {
    await write('\x1b]4;200;#abc\x07\x1b]12;rgb:123/4567/ab\x07\x1b]4;200;?\x07\x1b]12;?\x07')
    assert.deepEqual(replies.splice(0), ['\x1b]4;200;rgb:aaaa/bbbb/cccc\x1b\\', '\x1b]12;rgb:1212/4545/abab\x1b\\'])
    await write(reset + '\x1b]4;200;?\x07\x1b]12;?\x07')
    assert.deepEqual(replies.splice(0), ['\x1b]4;200;rgb:ffff/0000/d7d7\x1b\\', '\x1b]12;rgb:f5f5/e0e0/dcdc\x1b\\'])
  }
})

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
