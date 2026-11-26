import os from 'node:os'
import fs from 'node:fs/promises'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { verifyCodexToolHook } from './verify-codex-tool-hook'
import { hashPluginHook,scopedToolHookPlugins } from '../sync/plugin-snapshot'
import { executeHook, type HookExecuteRequest } from './executor'

const profile = vi.hoisted(() => ({ home: '', enabled: true }))
vi.mock('../bridge-state', () => ({ bridgeStatePath: (name: string) => path.join(profile.home, name) }))
vi.mock('../plugin-helpers', () => ({
  listEnabledInstalledPlugins: async ({cwd}:{cwd?:string}) =>
    profile.enabled && cwd !== 'other' ? [{ key: 'sample@local', installPath: profile.home, name: 'sample', marketplace: 'local' }] : [],
  readEffectivePluginManifest: async () => ({}),
}))
vi.mock('../plugin-monitors', () => ({}))
const handler = { type: 'command', command: 'reviewed-guard', args: ['sample'], timeout: 2 }
const group = { matcher: 'Read', hooks: [handler] }
const hooks = { PreToolUse: [group] }
const request: HookExecuteRequest = {
  type: 'hook:execute',
  sessionId: 'test',
  requestId: '1',
  hookId: 'example',
  event: 'PreToolUse',
  command: handler.command,
  args: handler.args,
  timeoutMs: 2000,
  payload: {},
}
let settings: string
beforeEach(async () => {
  profile.home = await fs.mkdtemp(path.join(os.tmpdir(), 'verified-hook-'))
  profile.enabled = true
  vi.spyOn(os, 'homedir').mockReturnValue(profile.home)
  await fs.mkdir(path.join(profile.home, '.claude'))
  settings = path.join(profile.home, '.claude', 'settings.json')
})
afterEach(async () => {
  vi.restoreAllMocks()
  await fs.rm(profile.home, { recursive: true, force: true })
})

describe('local Codex command hook verification', () => {
  it('checks the entire current user declaration including event, arguments, matcher and timeout', async () => {
    const declaration = { source: 'user' as const, matcher: group.matcher, handler }
    await fs.writeFile(settings, JSON.stringify({ hooks }))
    await expect(verifyCodexToolHook(request, declaration)).resolves.toBeUndefined()
    for (const change of [{ command: 'other' }, { args: ['other'] }, { timeoutMs: 3000 }, { event: 'PostToolUse' }]) {
      await expect(verifyCodexToolHook({ ...request, ...change }, declaration)).rejects.toThrow()
    }
    await expect(verifyCodexToolHook(request, { ...declaration, matcher: 'Write' })).rejects.toThrow()
    await fs.writeFile(settings, JSON.stringify({ hooks, disableAllHooks: true }))
    await expect(verifyCodexToolHook(request, declaration)).rejects.toThrow()
  })

  it('runs an approved command on the actual client host with native paths and receives its decision', async () => {
    const fixture = path.join(profile.home, 'sample hook.cjs')
    await fs.writeFile(
      fixture,
      "const value=JSON.parse(require('node:fs').readFileSync(0,'utf8')); console.log(JSON.stringify({hookSpecificOutput:{hookEventName:value.hook_event_name,permissionDecision:value.tool_input.file_path==='restricted.txt'?'deny':'allow'}}))",
    )
    const command = { type: 'command', command: process.execPath, args: [fixture], timeout: 5 }
    await fs.writeFile(settings, JSON.stringify({ hooks: { PreToolUse: [{ matcher: 'Read', hooks: [command] }] } }))
    const declaration = { source: 'user' as const, matcher: 'Read', handler: command }
    for (const [file, decision] of [
      ['restricted.txt', 'deny'],
      ['public.txt', 'allow'],
    ]) {
      const req = {
        ...request,
        command: command.command,
        args: command.args,
        timeoutMs: 5000,
        cwd: profile.home,
        payload: { hook_event_name: 'PreToolUse', tool_name: 'Read', tool_input: { file_path: file } },
      }
      await verifyCodexToolHook(req, declaration)
      expect(await executeHook(req)).toMatchObject({
        exitCode: 0,
        outcome: 'success',
        jsonOutput: { hookSpecificOutput: { permissionDecision: decision } },
      })
    }
  })

  it('checks enabled plugins and the approval in the current host profile before each call', async () => {
    const declaration = { source: 'plugin' as const, matcher: group.matcher, handler }
    const pluginRequest = { ...request, pluginId: 'sample@local' }
    const approvals = path.join(profile.home, 'plugin-hook-approvals.json')
    await fs.writeFile(path.join(profile.home, 'hooks.json'), JSON.stringify({ hooks }))
    await expect(verifyCodexToolHook(pluginRequest, declaration)).rejects.toThrow()
    await fs.writeFile(
      approvals,
      JSON.stringify({ 'sample@local': [hashPluginHook(request.event, group.matcher, handler)] }),
    )
    await expect(verifyCodexToolHook(pluginRequest, declaration,'allowed')).resolves.toBe(profile.home)
    await expect(verifyCodexToolHook({...pluginRequest,cwd:'allowed'}, declaration,'other')).rejects.toThrow()
    expect(await scopedToolHookPlugins('other')).toEqual([])
    expect(await scopedToolHookPlugins('allowed')).toMatchObject([{id:'sample@local',hooks}])
    profile.enabled = false
    await expect(verifyCodexToolHook(pluginRequest, declaration)).rejects.toThrow()
    profile.enabled = true
    await fs.rm(approvals)
    await expect(verifyCodexToolHook(pluginRequest, declaration)).rejects.toThrow()
  })
})
