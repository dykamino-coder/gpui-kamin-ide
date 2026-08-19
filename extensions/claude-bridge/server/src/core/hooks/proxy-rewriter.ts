// Take user's HookSettings → produce CLI-bound HookSettings where every
// local command hook is replaced by an exec-form Node relay pointing at our
// /api/hooks/<session>/<event>/<id> endpoint. The relay reproduces the local
// command's stdout, stderr, and exit code inside the Linux CLI process. This
// lets Claude Code apply its native, event-specific command-hook semantics
// (including exit 2) instead of trying to translate them through HTTP hooks.
// Server-side command hooks are kept as-is. Prompt/agent/http hooks pass
// through unchanged — CLI handles them natively.

import { config } from '../config'
import type {
  HookHandler,
  HookMatcher,
  HookSettings,
  HookSource,
} from './types'
import { registerSessionHooks } from './registry'

export function buildHookCommandRelay(url: string, hookProxyToken: string): string {
  return [
    'let d="";',
    'process.stdin.setEncoding("utf8");',
    'process.stdin.on("data",c=>d+=c);',
    'process.stdin.on("end",async()=>{try{',
    `const r=await fetch(${JSON.stringify(url)},{method:"POST",headers:{"Content-Type":"application/json","Authorization":${JSON.stringify(`Bearer ${hookProxyToken}`)},"X-Bridge-Hook-Relay":"command"},body:d});`,
    'const t=await r.text();',
    'if(!r.ok){if(t)process.stderr.write(t);process.exitCode=1;return}',
    'let j;try{j=JSON.parse(t)}catch{process.stderr.write("Invalid hook relay response");process.exitCode=1;return}',
    'const o=j&&j.result;',
    'if(!o||typeof o!=="object"){process.stderr.write("Missing hook relay result");process.exitCode=1;return}',
    'if(typeof o.stdout==="string")process.stdout.write(o.stdout);',
    'if(typeof o.stderr==="string")process.stderr.write(o.stderr);',
    'const x=Number(o.exitCode);process.exitCode=Number.isInteger(x)?x:1;',
    '}catch(e){process.stderr.write(String(e));process.exitCode=1}});',
  ].join('')
}

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

        // Keep command-hook semantics on both sides of the bridge. Exec-form
        // avoids quoting/token expansion in the container and works for sync,
        // async, and asyncRewake without event-specific HTTP translations.
        const relay: HookHandler = {
          type: 'command',
          command: 'node',
          args: ['-e', buildHookCommandRelay(proxyUrl, hookProxyToken)],
          timeout: h.timeout,
          if: h.if,
          statusMessage: h.statusMessage,
          once: h.once,
        }
        if (h.async || h.asyncRewake) {
          relay.async = true
          relay.asyncRewake = h.asyncRewake
        }
        return relay
      })
      return { matcher: m.matcher, hooks: newHandlers }
    })
    ;(out as Record<string, unknown>)[event] = newMatchers
  }
  return out
}
