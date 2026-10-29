import { afterEach, expect, it, vi } from 'vitest'
const fixture = vi.hoisted(() => ({ shells: vi.fn(), stop: vi.fn() }))
vi.mock('../mcp/tools/shell', () => ({
  localBackgroundShellSnapshot: fixture.shells,
  stopLocalBackgroundShell: fixture.stop,
}))
import { ClientCommands } from './client-commands'

afterEach(() => {
  vi.clearAllMocks()
  vi.restoreAllMocks()
})
const shell = { id: 'shell:1', tool: 'PowerShell', command: 'legacy', startedAt: 1, running: true, output: 'Привет' }
const task = { id: 'task:2', tool: 'Bash', command: 'task', startedAt: 2, background: true, status: 'running' as const }
it('lists, reads and stops local legacy shells without any server snapshot', () => {
  fixture.shells.mockImplementation((tab) => (tab === 'a' ? [shell] : []))
  const publish = vi.fn()
  const commands = new ClientCommands('a', publish)
  commands.list('old-server')
  expect(publish).toHaveBeenLastCalledWith(
    'commands:snapshot',
    expect.objectContaining({
      serverAvailable: false,
      commands: [expect.objectContaining({ id: shell.id, background: true })],
    }),
  )
  expect(commands.localAction('output', shell.id)).toBe(true)
  expect(publish).toHaveBeenLastCalledWith('commands:output', { id: shell.id, output: 'Привет' })
  expect(commands.localAction('stop', shell.id)).toBe(true)
  expect(fixture.stop).toHaveBeenCalledWith('a', shell.id)
  expect(new ClientCommands('b', publish).localAction('stop', shell.id)).toBe(false)
  expect(commands.localAction('stop', task.id)).toBe(false)
})
it('merges server tasks without duplicate or stale legacy shells, and expires unavailable snapshots', () => {
  fixture.shells.mockReturnValue([shell])
  const publish = vi.fn()
  const commands = new ClientCommands('a', publish)
  commands.receive({ sessionId: 's', commands: [task, { ...task, id: shell.id }] })
  expect(publish.mock.lastCall?.[1]).toMatchObject({
    serverAvailable: true,
    commands: [{ id: shell.id, command: 'legacy' }, task],
  })
  fixture.shells.mockReturnValue([{ ...shell, running: false }])
  commands.list('s')
  expect(publish.mock.lastCall?.[1].commands).toEqual([task])
  commands.list('replacement')
  expect(publish.mock.lastCall?.[1]).toMatchObject({ serverAvailable: false, commands: [] })
  vi.spyOn(Date, 'now').mockReturnValue(Date.now() + 6000)
  commands.list('s')
  expect(publish.mock.lastCall?.[1].serverAvailable).toBe(false)
  commands.reset()
  commands.list('s')
  expect(publish.mock.lastCall?.[1].commands).toEqual([])
})
