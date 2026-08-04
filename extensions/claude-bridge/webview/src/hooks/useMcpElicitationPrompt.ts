// MCP server-initiated elicitation handler.
//
// When a remote MCP server needs input mid-call it sends `elicitation/create`.
// We surface it as a NON-BLOCKING widget (McpElicitationWidget) in the widget
// panel — the widget owns the schema form + responds via respondMcpElicitation.
// Previously this used native `window.prompt`, which SYNCHRONOUSLY froze the
// whole renderer thread (and the shared host window) until dismissed.

import { useEffect } from 'preact/hooks'
import type { ElectronBridge } from '../../shared/types'
import { activeWidgets } from '../signals/widgets'

export function useMcpElicitationPrompt(bridge: ElectronBridge): void {
  useEffect(() => {
    return bridge.onMcpElicitation((req) => {
      // Replace any older unresolved request from the same server, then queue
      // this one as a widget. The widget's Submit/Cancel call respondMcpElicitation.
      const filtered = activeWidgets.value.filter(w => w.requestId !== req.requestId)
      activeWidgets.value = [...filtered, {
        requestId: req.requestId,
        resolved: false,
        type: 'mcpElicitation',
        data: {
          requestId: req.requestId,
          serverName: req.serverName,
          message: req.message,
          requestedSchema: req.requestedSchema,
        },
      }]
    })
  }, [])
}
