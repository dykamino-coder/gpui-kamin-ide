// ============================================================================
// Error Handler Middleware
// Catches unhandled errors from downstream handlers and returns JSON errors.
// ============================================================================

import type { Context, Next } from "hono"
import { errorLog } from "../logging"

/**
 * Hono middleware that catches thrown errors and returns formatted JSON.
 * Must be registered BEFORE route handlers so it wraps them.
 */
export async function errorHandlerMiddleware(c: Context, next: Next) {
  try {
    await next()
  } catch (err) {
    const errorMessage = err instanceof Error ? err.message : "Unknown error"

    errorLog("Request error caught by middleware", {
      error: errorMessage,
      path: c.req.path,
    })

    return c.json(
      {
        type: "error",
        error: {
          type: "server_error",
          message: errorMessage,
        },
      },
      500
    )
  }
}
