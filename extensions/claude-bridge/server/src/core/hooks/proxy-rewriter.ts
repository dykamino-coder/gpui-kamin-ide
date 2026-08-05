// Take user's HookSettings → produce CLI-bound HookSettings where every
// command-type hook with effectiveHost ∈ {'local','http'} is replaced by
// an `http`-type hook pointing at our /api/hooks/<session>/<event>/<id>
// proxy endpoint. Server-side command hooks are kept as-is (CLI runs
// them inside the container). Prompt/agent/http hooks always pass
// through unchanged — CLI handles them natively.

import { config } from '../config'
import type {
  HookHandler,
  HookMatcher,
  HookSettings,
  HookSource,
} from './types'
import { registerSessionHooks } from './registry'

/**
 * Rewrite the user's hooks for a session and return the JSON shape that
 * goes into the CLI's settings.json:hooks. Side-effect: registers each
 * rewritten hook in the per-session registry so incoming webhook calls
 * can be matched back to their config.
 */
export function rewriteHooksForCli(
  sessionId: string,
  hooks: HookSettings,
  sourceFn: HookSource | ((m: HookMatcher) => HookSource),
  hookProxyToken: string,
): HookSettings {
  const annotated = registerSessionHooks(sessionId, hooks, sourceFn)
  const out: HookSettings = {}
  const proxyOrigin = `http://127.0.0.1:${config.port}`

  const asyncProxyCommand = (url: string): string => {
    const script = [
      'let d="";',
      'process.stdin.setEncoding("utf8");',
      'process.stdin.on("data",c=>d+=c);',
      `process.stdin.on("end",async()=>{try{const r=await fetch(${JSON.stringify(url)},{method:"POST",headers:{"Content-Type":"application/json","Authorization":${JSON.stringify(`Bearer ${hookProxyToken}`)}},body:d});const t=await r.text();if(t)process.stdout.write(t);const x=Number(r.headers.get("x-bridge-hook-exit-code"));process.exit(Number.isFinite(x)?x:(r.ok?0:1))}catch(e){process.stderr.write(String(e));process.exit(1)}});`,
    ].join('')
    return `node -e "eval(Buffer.from('${Buffer.from(script).toString('base64')}','base64').toString('utf8'))"`
  }

  for (const [event, matchers] of Object.entries(hooks)) {
    if (!Array.isArray(matchers)) continue
    const newMatchers = matchers.map(m => {
      if (!m || !Array.isArray(m.hooks)) return m
      const newHandlers: HookHandler[] = m.hooks.map(h => {
        const reg = annotated.get(h)
        if (!reg) return h

        // Pass-through for non-command types (LLM and HTTP run native).
        if (h.type !== 'command') return h

        // Server-side command hook — keep as-is, CLI runs it in container.
        if (reg.effectiveHost === 'server') return h

        const proxyUrl = `${proxyOrigin}/api/hooks/${sessionId}/${encodeURIComponent(event)}/${reg.id}`

        // Async is command-only in Claude Code. Keep it a command and run a
        // tiny container-side Node relay in the background; the real command
        // still executes on the client host through the authenticated proxy.
        if (h.async || h.asyncRewake) {
          return {
            type: 'command',
            command: asyncProxyCommand(proxyUrl),
            timeout: h.timeout,
            if: h.if,
            statusMessage: h.statusMessage,
            once: h.once,
            async: true,
            asyncRewake: h.asyncRewake,
          }
        }

        // Synchronous local → rewrite as http hook pointing at our proxy.
        // CLI will POST the hook input JSON; the proxy looks up the original
        // command, dispatches per host, returns CLI-compatible response.
        return {
          type: 'http',
          url: proxyUrl,
          headers: {
            Authorization: `Bearer ${hookProxyToken}`,
          },
          timeout: h.timeout,
          if: h.if,
          statusMessage: h.statusMessage,
          once: h.once,
        }
      })
      return { matcher: m.matcher, hooks: newHandlers }
    })
    ;(out as Record<string, unknown>)[event] = newMatchers
  }
  return out
}
