// ============================================================================
// Dashboard authorization helpers — per-caller scoping on top of the auth
// middleware (dashboardAuthMiddleware stamps `authKind` + `apiUserName`).
//
// Rule enforced here: a per-user API token ('api' authKind) may read ONLY its
// own data and may NOT perform global/admin mutations. An admin dashboard
// session ('admin') and the auth-disabled local mode (no authKind) are left
// untouched — so this tightens the multi-user attack surface without changing
// the single-user / admin experience (no lockout risk).
// ============================================================================

import type { Context } from 'hono'

/** authKind stamped by the middleware, read through a cast (no Hono Variables
 *  map is declared project-wide). 'admin' = dashboard session, 'api' = per-user
 *  token, undefined = auth disabled. */
export function authKind(c: Context): 'admin' | 'api' | undefined {
  return c.get('authKind' as never) as 'admin' | 'api' | undefined
}

/** The userName a per-user API token resolved to (undefined for admin/no-auth). */
export function apiUserName(c: Context): string | undefined {
  return c.get('apiUserName' as never) as string | undefined
}

/** The tokenId a per-user API token resolved to (undefined for admin/no-auth). */
export function apiTokenId(c: Context): string | undefined {
  return c.get('apiTokenId' as never) as string | undefined
}

/** True when the caller is a per-user API token whose access must be scoped. */
export function isApiScoped(c: Context): boolean {
  return authKind(c) === 'api'
}

/** Guard for admin-only routes (global config, destructive maintenance).
 *  Returns a 403 Response when the caller is a per-user API token; otherwise
 *  null (admin session and auth-disabled mode pass through). */
export function denyApiToken(c: Context): Response | null {
  if (isApiScoped(c)) {
    return c.json({ error: 'Forbidden: admin privileges required' }, 403)
  }
  return null
}

/** Guard for per-user data routes. Returns a 403 Response when a per-user API
 *  token targets someone else's data; otherwise null (admin/no-auth, or an api
 *  token targeting its own userName, pass through). Comparison is exact. */
export function denyForeignUser(c: Context, targetUserName: string | undefined | null): Response | null {
  if (isApiScoped(c) && (!targetUserName || targetUserName !== apiUserName(c))) {
    return c.json({ error: 'Forbidden: not your data' }, 403)
  }
  return null
}

/** Guard for routes keyed by an ambiguous subject param that may be a tokenId
 *  OR a userName (some legacy endpoints bind the same param to either column).
 *  Passes when a per-user token's own tokenId or userName matches; otherwise
 *  403. Admin/no-auth always pass. */
export function denyForeignSubject(c: Context, subject: string | undefined | null): Response | null {
  if (isApiScoped(c) && subject !== apiTokenId(c) && subject !== apiUserName(c)) {
    return c.json({ error: 'Forbidden: not your data' }, 403)
  }
  return null
}
