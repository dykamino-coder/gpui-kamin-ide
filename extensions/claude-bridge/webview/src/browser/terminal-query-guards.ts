import type { IParser } from '@xterm/xterm'

/** The host emulator answers terminal queries once, even with zero pages.
 * Replica renderers must not send the same parser responses back as input. */
export function guardTerminalQueries(parser: IParser): () => void {
  const guards = [
    parser.registerCsiHandler({ final: 'c' }, () => true),
    parser.registerCsiHandler({ prefix: '>', final: 'c' }, () => true),
    parser.registerCsiHandler({ final: 'n' }, () => true),
    parser.registerCsiHandler({ prefix: '?', final: 'n' }, () => true),
    parser.registerCsiHandler({ intermediates: '$', final: 'p' }, () => true),
    parser.registerCsiHandler({ prefix: '?', intermediates: '$', final: 'p' }, () => true),
    parser.registerCsiHandler({ final: 't' }, params => Number(params[0]) >= 11 && Number(params[0]) <= 21),
    parser.registerDcsHandler({ intermediates: '$', final: 'q' }, () => true),
  ]
  return () => { for (const guard of guards) guard.dispose() }
}
