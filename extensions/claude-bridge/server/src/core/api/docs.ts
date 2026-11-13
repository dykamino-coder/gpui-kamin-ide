import type { Hono, MiddlewareHandler } from 'hono'
import { serveStatic } from '@hono/node-server/serve-static'
import { dashboardAuthMiddleware } from '../middleware/dashboard-auth'
import { createSpecification } from './registry'

export function registerDocumentation(app: Hono): void {
  const administrator: MiddlewareHandler = async (c, next) => {
    const result = await dashboardAuthMiddleware(c, async () => {
      if (c.get('authKind') === 'api') {
        c.res = c.json({ error: 'Administrator session required' }, 403)
        return
      }
      await next()
    })
    if (result?.status === 401) c.header('WWW-Authenticate', 'Basic realm="Bridge API"')
    return result
  }
  app.get('/api/openapi.json', administrator, (c) => c.json(createSpecification(app)))
  app.get('/api/docs', administrator, serveStatic({ path: './dist/dashboard/api-docs.html' }))
}
