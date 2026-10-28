import { describe, expect, it } from 'vitest'
import { runForegroundCommand } from './shell-foreground'
describe('foreground shell task cancellation', () => {
  it('does not spawn a command whose request was already cancelled', async () => {
    const controller = new AbortController()
    controller.abort()
    const result = await runForegroundCommand(
      'throw new Error("must not run")',
      process.execPath,
      10_000,
      ['-e'],
      process.env,
      controller.signal,
    )
    expect(result.content[0]!.text).toContain('cancelled')
  })
  it('terminates its own process tree, including a child ignoring TERM', async () => {
    const controller = new AbortController()
    const code = `const {spawn}=require('node:child_process');const child=spawn(process.execPath,['-e',"process.on('SIGTERM',()=>{});setInterval(()=>{},1000)"],{stdio:'inherit',windowsHide:true});console.log('child='+child.pid);setInterval(()=>{},1000)`
    const timer = setTimeout(() => controller.abort(), 1000)
    const result = await runForegroundCommand(code, process.execPath, 10_000, ['-e'], process.env, controller.signal)
    clearTimeout(timer)
    expect(result.content[0]!.text).toContain('Command cancelled')
    const pid = Number(/child=(\d+)/.exec(result.content[0]!.text)?.[1])
    expect(pid).toBeGreaterThan(0)
    await new Promise((resolve) => setTimeout(resolve, 1200))
    expect(() => process.kill(pid, 0)).toThrow()
  }, 10_000)
})
