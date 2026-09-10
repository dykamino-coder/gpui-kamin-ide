import { WebSocket as WS } from 'ws'

type ErrorSocket = Pick<WS, 'readyState' | 'send' | 'close'>

/** `session:error` is terminal for the current WS/session binding. Send the
 * structured reason first, then close so every client must re-authenticate and
 * cannot keep writing into a missing PTY. */
export function sendSessionError(
  ws: ErrorSocket,
  error: string,
  closeCode = 1011,
  closeReason = 'Session error',
): void {
  if (ws.readyState !== WS.OPEN) return
  ws.send(JSON.stringify({ type: 'session:error', error, fatal: true }))
  ws.close(closeCode, closeReason)
}
