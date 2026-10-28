// Spawn only synthetic Node programs. Never load a user shell or profile.
import { describe, expect, it } from 'vitest'
import { runForegroundCommand } from './shell-foreground'

describe('foreground shell local process acceptance', () => {
  it('settles a timed-out process tree without loading a user shell', async () => {
    const program = `
      const { spawn } = require('node:child_process');
      const child = spawn(process.execPath, ['-e', 'setTimeout(() => {}, 15000)'], {
        stdio: ['ignore', process.stdout, process.stderr], windowsHide: true
      });
      console.log('synthetic-descendant=' + child.pid);
      child.unref();
      ${process.platform === 'win32' ? 'setTimeout(() => {}, 15000);' : ''}
    `
    const start = Date.now()
    const result = await runForegroundCommand(program, process.execPath, 300, ['-e'], {})
    const text = result.content.map(item => item.text).join('')
    const ownPid = Number(/synthetic-descendant=(\d+)/.exec(text)?.[1])
    // This PID comes only from the test program we just launched.
    try {
      expect(ownPid).toBeGreaterThan(0)
      expect(text).toContain('timed out')
      expect(Date.now() - start).toBeLessThan(4000)
    } finally {
      if (ownPid > 0) { try { process.kill(ownPid, 'SIGKILL') } catch { /* tree termination already succeeded */ } }
    }
  }, 6000)

  it('drains a large output while retaining bounded stdout and stderr', async () => {
    const program = `process.stdout.write('x'.repeat(2 * 1024 * 1024)); process.stderr.write('y'.repeat(2 * 1024 * 1024));`
    const result = await runForegroundCommand(program, process.execPath, 3000, ['-e'], {})
    const text = result.content.map(item => item.text).join('')
    expect(Buffer.byteLength(text)).toBeLessThan(206 * 1024)
    expect(text).toContain('Output truncated')
    expect(text).not.toContain('timed out')
  })
})
