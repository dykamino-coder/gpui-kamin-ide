// What Send does with the composer's text for the tab's connection state.
//
// While the chat reconnects (a Bridge server restart, a network blip) the
// message goes to the host: it waits there for the chat's own conversation and
// is sent once it is back, or returns to the composer with the reason. Before,
// Send did nothing at all in that state and the user was not told why.

export type SendGate = 'send' | 'wait' | 'not-connected'

export const NOT_CONNECTED_WARNING =
  'Message not sent: the chat is not connected. Your text stays in the composer; reconnect the chat to send it.'

/** `notReady`: the view is switching tabs or still downloading the transcript
 *  (the text stays; Send works a moment later). */
export function sendGate(status: string, notReady: boolean): SendGate {
  if (notReady) return 'wait'
  if (status === 'disconnected' || status === 'error') return 'not-connected'
  return 'send'
}
