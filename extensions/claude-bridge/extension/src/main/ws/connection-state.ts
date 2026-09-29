import type { ConnectionState } from '../../shared/types'
import type { ExtendedConnectionState } from './connection-manager'

/** Single mapper for live events and list snapshots. Keeping it pure prevents
 * status/session/retry fields from drifting between the two IPC paths. */
export function toRendererConnectionState(
  state: ExtendedConnectionState,
  authority: string,
  authorityGeneration: number,
  authoritySequence: number,
  revision: number,
): ConnectionState {
  return {
    status: state.status === 'authenticated' ? 'connected'
      : state.status === 'connected' ? 'connecting'
      : state.status as ConnectionState['status'],
    authority,
    authorityGeneration,
    authoritySequence,
    revision,
    sessionId: state.sessionId,
    error: state.error,
    nextRetryAt: state.nextRetryAt,
    retryAttempt: state.retryAttempt,
    // Diagnostics classify a disconnect by this number when the server closed
    // without a reason text — dropping it here turned every such close into an
    // unknown cause downstream (INC-2026-0027).
    closeCode: state.closeCode,
  }
}
