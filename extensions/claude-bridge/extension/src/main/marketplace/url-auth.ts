// Helpers for embedding and redacting HTTP credentials in marketplace URLs.
//
// Approach: when the user supplies a token in the Add Marketplace form, we
// bake it straight into the URL and write that URL into
// `~/.claude/plugins/known_marketplaces.json`. Claude Code CLI reads the
// same file, so a `git clone` / `pull` from the CLI uses the same token
// without any extra setup on the user's machine. That file is user-only
// (chmod 600-ish on POSIX, per-user ACL on Windows) so plaintext there is
// acceptable — same bar as `~/.git-credentials` with the `store` helper.
//
// We never print the baked-in URL to logs or the renderer without passing
// through `redactUrl` first.

export interface MarketplaceAuth {
  /** Username placed before `:<token>@` in the URL. Defaults to `oauth2`
   *  which GitLab accepts for PAT auth; GitHub works with any string. */
  username: string
  token: string
}

/** Embed auth into an HTTP(S) URL. Leaves SSH / git:// / local paths alone. */
export function applyAuthToUrl(url: string, auth: MarketplaceAuth): string {
  let parsed: URL
  try { parsed = new URL(url) } catch { return url }
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') return url
  parsed.username = encodeURIComponent(auth.username)
  parsed.password = encodeURIComponent(auth.token)
  return parsed.toString()
}

/** Hide credentials in a URL for display / logging: `http://user:pass@host` →
 *  `http://user:***@host`. Mirrors CLI's redactUrlCredentials behaviour.
 *
 *  Only for a string that IS a URL. Free text goes through `redactUrlsInText`:
 *  `new URL` happily reads `fatal: unable to access https://user:token@host` as
 *  a URL with scheme `fatal:` and no password, and would hand the token back
 *  untouched (INC-2026-0041). */
export function redactUrl(url: string): string {
  try {
    const u = new URL(url)
    if (u.password) u.password = '***'
    return u.toString()
  } catch {
    return redactUrlsInText(url)
  }
}

/** Hide credentials in every URL embedded in arbitrary text — a git error
 *  message, a stderr dump, a log line. Purely textual on purpose: whole-string
 *  URL parsing cannot be trusted here, see `redactUrl`. */
export function redactUrlsInText(text: string): string {
  return text.replace(/(https?:\/\/)([^@\s/]*):([^@\s/]*)@/gi, '$1$2:***@')
}
