import assert from 'node:assert/strict'
import { once, type EventEmitter } from 'node:events'
import type { WebSocket } from 'ws'

export async function waitFrame(frames: any[], events: EventEmitter, predicate: (value: any) => boolean) {
  const signal = AbortSignal.timeout(5000)
  while (true) {
    const result = frames.find(predicate)
    if (result) return result
    await once(events, 'frame', { signal })
  }
}

export async function assertTitleRegeneration(
  socket: WebSocket,
  invoke: (channel: string) => Promise<any>,
  upstreamFrames: any[],
  upstreamEvents: EventEmitter,
): Promise<void> {
  // Same header action and shared handler as the desktop webview, without
  // loading native SessionsBridge. Malformed/stale tabs must not reach the CLI.
  const activeTab = (await invoke('tab:get-active')).result
  for (const tabId of [null, {}, 'missing-tab', activeTab]) {
    socket.send(JSON.stringify({ kind: 'send', channel: 'bridge:regenerate-title', args: [tabId] }))
  }
  await waitFrame(upstreamFrames, upstreamEvents, frame => frame.type === 'session:submitText' && frame.data === '/rename')
  await invoke('get-version') // Ordered ingress barrier after the header action.
  assert.equal(upstreamFrames.filter(frame => frame.type === 'session:submitText' && frame.data === '/rename').length, 1)
}
