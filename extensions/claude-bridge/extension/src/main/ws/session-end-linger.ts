import WebSocket from 'ws'

/** How long an ended tab keeps its socket for the session's own `SessionEnd`
 *  hooks (BR-18A). The server keeps an exiting session resolvable for the same
 *  5 s (`TEARDOWN_GRACE_MS` in the server's `session-core.ts`). */
export const SESSION_END_HOOK_WINDOW_MS = 5000

type Socket = Pick<WebSocket, 'readyState' | 'send' | 'close' | 'on' | 'removeAllListeners'>

/** Keep an ended session's socket open just long enough to run the local hooks
 *  the server relays while the CLI exits (`SessionEnd`), then close it.
 *
 *  The tab already sent `session:end` and is gone; nothing else is read from
 *  the socket and it never reconnects. Closing in the same tick as
 *  `session:end` made the server's relay find no client, so a local
 *  `SessionEnd` hook never ran on a tab close. The socket closes when the
 *  server closes it, or when the window ends and no relayed hook is still
 *  running. */
export function lingerForSessionEnd(
  sock: Socket,
  runHook: (msg: unknown, send: (reply: unknown) => void) => Promise<void>,
  windowMs = SESSION_END_HOOK_WINDOW_MS,
): void {
  let running = 0
  let windowOver = false
  let closed = false
  const close = () => {
    if (closed) return
    closed = true
    clearTimeout(timer)
    if (sock.readyState === WebSocket.OPEN || sock.readyState === WebSocket.CONNECTING) sock.close()
  }
  const send = (reply: unknown) => {
    if (sock.readyState === WebSocket.OPEN) sock.send(JSON.stringify(reply))
  }
  sock.removeAllListeners()
  sock.on('error', () => {
    /* teardown: the socket is closing anyway */
  })
  sock.on('message', (raw: WebSocket.RawData) => {
    let msg: { type?: string } | undefined
    try {
      msg = JSON.parse(raw.toString()) as { type?: string }
    } catch {
      return
    }
    if (msg?.type !== 'hook:execute' || closed) return
    running++
    void runHook(msg, send).finally(() => {
      running--
      if (windowOver && running === 0) close()
    })
  })
  sock.on('close', () => {
    closed = true
    clearTimeout(timer)
  })
  const timer = setTimeout(() => {
    windowOver = true
    if (running === 0) close()
  }, windowMs)
}
