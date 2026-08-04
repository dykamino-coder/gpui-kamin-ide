// ============================================================================
// EventBus - Real-time event system for dashboard updates
// ============================================================================

export type BridgeEventType =
  | 'health:updated'
  | 'account:updated'
  | 'terminal:sessions'
  | 'usage:updated'
  | 'session:created'
  | 'session:destroyed'
  | 'session:updated'
  | 'session:tree-updated'
  | 'session:reaped'
  | 'mcp:call'
  | 'request:start'
  | 'request:complete'
  | 'request:error'
  | 'elicitation:pending'
  | 'elicitation:resolved'

export interface BridgeEvent {
  type: BridgeEventType
  data: unknown
  timestamp: number
}

class EventBus {
  private listeners = new Map<string, Set<(event: BridgeEvent) => void>>()
  private broadcastFn: ((event: BridgeEvent) => void) | null = null

  /**
   * Emit an event to all matching listeners and the broadcast function.
   * Errors in individual listeners are caught so they never break other listeners.
   */
  emit(type: BridgeEventType, data: unknown): void {
    const event: BridgeEvent = { type, data, timestamp: Date.now() }

    // Send to listeners subscribed to this specific event type
    const typeListeners = this.listeners.get(type)
    if (typeListeners) {
      for (const handler of typeListeners) {
        try {
          handler(event)
        } catch (_err) {
          // Swallow listener errors to avoid breaking other listeners
        }
      }
    }

    // Send to wildcard '*' listeners
    const wildcardListeners = this.listeners.get('*')
    if (wildcardListeners) {
      for (const handler of wildcardListeners) {
        try {
          handler(event)
        } catch (_err) {
          // Swallow listener errors
        }
      }
    }

    // Forward to broadcast function (set by WebSocket server)
    if (this.broadcastFn) {
      try {
        this.broadcastFn(event)
      } catch (_err) {
        // Swallow broadcast errors
      }
    }
  }

  /**
   * Subscribe to events of a specific type, or '*' for all events.
   * Returns an unsubscribe function.
   */
  on(type: BridgeEventType | '*', handler: (event: BridgeEvent) => void): () => void {
    if (!this.listeners.has(type)) {
      this.listeners.set(type, new Set())
    }

    const handlers = this.listeners.get(type)!
    handlers.add(handler)

    // Return unsubscribe function
    return () => {
      handlers.delete(handler)
      if (handlers.size === 0) {
        this.listeners.delete(type)
      }
    }
  }

  /**
   * Set the broadcast function used by the WebSocket server to forward
   * ALL events to connected clients.
   */
  setBroadcast(fn: (event: BridgeEvent) => void): void {
    this.broadcastFn = fn
  }
}

export const eventBus = new EventBus()
