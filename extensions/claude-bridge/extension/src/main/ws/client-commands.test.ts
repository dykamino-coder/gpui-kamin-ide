import { afterEach, expect, it, vi } from 'vitest'
const fixture = vi.hoisted(() => ({
  shells: vi.fn(),
  stop: vi.fn(),
  clear: vi.fn(),
}))
vi.mock('../mcp/tools/shell', () => ({
  clearLocalCompletedShells: fixture.clear,
  localBackgroundShellSnapshot: fixture.shells,
  stopLocalBackgroundShell: fixture.stop,
}))
import { ClientCommands } from './client-commands'

afterEach(() => {
  vi.clearAllMocks()
  vi.restoreAllMocks()
})
const shell = {
  id: 'shell:1',
  tool: 'PowerShell',
  command: 'legacy',
  startedAt: 1,
  running: true,
  output: 'Привет',
}
const task = {
  id: 'task:2',
  tool: 'Bash',
  command: 'task',
  startedAt: 2,
  background: true,
  status: 'running' as const,
}
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
  expect(publish).toHaveBeenLastCalledWith('commands:output', {
    id: shell.id,
    output: 'Привет',
  })
  expect(commands.localAction('stop', shell.id)).toBe(true)
  expect(fixture.stop).toHaveBeenCalledWith('a', shell.id)
  expect(new ClientCommands('b', publish).localAction('stop', shell.id)).toBe(false)
  expect(commands.localAction('stop', task.id)).toBe(false)
})
it('merges server tasks without duplicate or stale legacy shells, and expires unavailable snapshots', () => {
  fixture.shells.mockReturnValue([shell])
  const publish = vi.fn()
  const commands = new ClientCommands('a', publish)
  commands.receive({
    sessionId: 's',
    commands: [task, { ...task, id: shell.id }],
  })
  expect(publish.mock.lastCall?.[1]).toMatchObject({
    serverAvailable: true,
    commands: [{ id: shell.id, command: 'legacy' }, task],
  })
  fixture.shells.mockReturnValue([{ ...shell, running: false }])
  commands.list('s')
  expect(publish.mock.lastCall?.[1].commands).toEqual([
    expect.objectContaining({ id: shell.id, status: 'failed' }),
    task,
  ])
  commands.list('replacement')
  expect(publish.mock.lastCall?.[1]).toMatchObject({
    serverAvailable: false,
    commands: [expect.objectContaining({ id: shell.id, status: 'failed' })],
  })
  vi.spyOn(Date, 'now').mockReturnValue(Date.now() + 6000)
  commands.list('s')
  expect(publish.mock.lastCall?.[1].serverAvailable).toBe(false)
  commands.reset()
  commands.list('s')
  expect(publish.mock.lastCall?.[1].commands).toEqual([expect.objectContaining({ id: shell.id, status: 'failed' })])
})

it('clears only finished records from both sources without losing active tasks or output', () => {
  const done = {
    ...shell,
    description: 'Legacy job',
    running: false,
    finishedAt: 9000,
    exitCode: 0,
  }
  fixture.shells.mockReturnValue([done, { ...shell, id: 'shell:2' }])
  const publish = vi.fn()
  const commands = new ClientCommands('a', publish)
  commands.receive({
    sessionId: 's',
    commands: [task, { ...task, id: 'done', status: 'stopped', finishedAt: 8000 }],
  })
  expect(publish.mock.lastCall?.[1].commands[0]).toMatchObject({
    description: 'Legacy job',
    status: 'completed',
    finishedAt: 9000,
    exitCode: 0,
  })
  fixture.clear.mockImplementation(() =>
    fixture.shells.mockReturnValue([
      { ...done, hiddenFromCommands: true },
      { ...shell, id: 'shell:2' },
    ]),
  )
  commands.clear()
  commands.list('s')
  expect(fixture.clear).toHaveBeenCalledWith('a')
  expect(publish.mock.lastCall?.[1].commands.map((item: { id: string }) => item.id)).toEqual(['shell:2', task.id])
  expect(commands.localAction('output', shell.id)).toBe(true)
  expect(publish).toHaveBeenLastCalledWith('commands:output', {
    id: shell.id,
    output: shell.output,
  })
})
