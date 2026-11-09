// Enumerated synthetic session peer only. File operations and view retention
// are the production WorkspaceFiles adapter, not fabricated replies.
import { existsSync } from 'node:fs'

export function filePanelPeer(root) {
  let active = 'one', attached = 0, switchSequence = 0
  const calls = [], unexpected = new Set()
  const tabs = ['one', 'two'].map(id => ({ id, cwd: root, label: id, folderName: id,
    createdAt: '2026-11-09T00:00:00Z', status: 'connected', sessionId: `fixture-${id}`,
    connectionAuthority: 'fixture', connectionAuthorityGeneration: 1, connectionAuthoritySequence: 1, connectionRevision: 1 }))
  const replies = new Map([
    ['get-version', 'fixture'], ['get-config', { serverUrl: 'ws://fixture.invalid', token: '[synthetic]', cwd: root }],
    ['layout:get', { filePanelVisible: true, filePanelBottomVisible: false }],
    ['skills:list', []], ['session:get-saved', []], ['tab:list', tabs], ['tabs:restore-state', []],
    ['vscode:is-available', false], ['mcp:status', []], ['mic-connected', false],
    ['get-editor-selection', null], ['bridge:server-version', null], ['get-machine-id', 'fixture'],
    ['hooks:list-pending-plugin-approvals', []], ['base-prompt:get', ''], ['base-prompt:set', true],
    ['bridge:server-fetch', { ok: false, status: 404, data: null }], ['notifications:get', {}], ['terminal:get-prefs', {}],
  ])
  return { calls, unexpected, get attached() { return attached }, backend: { attach(view) {
    attached++
    const event = (channel, ...args) => view.postMessage({ kind: 'event', channel, args })
    return {
      async receive(frame) {
        calls.push(frame)
        if (frame.kind === 'invoke') {
          const known = replies.has(frame.channel) || ['tab:get-active', 'fs:path-exists'].includes(frame.channel)
          if (!known) unexpected.add(frame.channel)
          await view.postMessage({ kind: 'invoke-reply', id: frame.id, generation: frame.generation,
            ok: known, ...(known ? { result: frame.channel === 'tab:get-active' ? active
              : frame.channel === 'fs:path-exists' ? existsSync(frame.args[0]) : replies.get(frame.channel) }
              : { error: 'Unsupported fixture channel' }) })
        } else if (frame.channel === 'workspace:sync') {
          await event('workspace:snapshot-start', tabs, active)
          await event('tab:list-changed', tabs)
          await event('jsonl-status', active, { status: 'watching', replayComplete: true })
          await event('config-ready', active)
          await event('workspace:snapshot-end')
        } else if (frame.channel === 'tab:switch') {
          active = frame.args[0]
          await event('tab:switched', active, ++switchSequence)
        }
      },
      dispose() { attached-- },
    }
  } } }
}
