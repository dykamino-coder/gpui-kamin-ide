// Take user's HookSettings → produce CLI-bound HookSettings where every
// local command hook is replaced by an exec-form Node relay pointing at our
// /api/hooks/<session>/<event>/<id> endpoint. The relay reproduces the local
// command's stdout, stderr, and exit code inside the Linux CLI process. This
// lets Claude Code apply its native, event-specific command-hook semantics
// (including exit 2) instead of trying to translate them through HTTP hooks.
// Server-side command hooks are kept as-is. Prompt/agent/http hooks pass
// through unchanged — CLI handles them natively.

import { config } from '../config'
import type { HookHandler, HookMatcher, HookSettings, HookSource } from './types'
import { registerSessionHooks } from './registry'

/** Env var the CLI process carries for us. The relay credential travels in
 *  the session's own process environment, never in the command text: the
 *  rewritten declaration is shown in Console, JSONL and the hooks panel, and a
 *  Bearer value interpolated there would be readable in all three. Hook
 *  children inherit the CLI's environment, so the relay still authenticates. */
export const HOOK_RELAY_TOKEN_ENV = 'CLAUDE_BRIDGE_HOOK_TOKEN'

export function buildHookCommandRelay(
  url: string,
  context = { event: 'unknown', type: 'command', host: 'local' },
): string {
  const safeLabel = (value: string) => (/^[A-Za-z][A-Za-z0-9_]{0,63}$/.test(value) ? value : 'unknown')
  const labels = { event: safeLabel(context.event), type: safeLabel(context.type), host: safeLabel(context.host) }
  // Отдельный процесс на каждый вызов: пул соединений здесь не нужен.
  // У fetch есть скрытый 300-секундный headersTimeout, тогда как dispatcher
  // и CLI разрешают хуку работать 600 секунд. Сроком управляет исполнитель.
  return [
    `const context=${JSON.stringify(labels)};`,
    // Сообщения исключений и HTTP body могут содержать URL, заголовки и
    // credential. Выводим только контекст и структурированные коды причин.
    'const diagnostic=e=>{const fields=[];const seen=new Set();',
    'const visit=(v,depth)=>{if(!v||typeof v!=="object"||seen.has(v)||depth>4)return;seen.add(v);',
    'for(const key of ["code","errno"]){const value=v[key];',
    'if(typeof value==="number"&&Number.isFinite(value)||typeof value==="string"&&/^[A-Z][A-Z0-9_]{0,63}$/.test(value))fields.push(key+"="+value)}',
    'visit(v.cause,depth+1);if(Array.isArray(v.errors))for(const item of v.errors.slice(0,4))visit(item,depth+1)};',
    'visit(e,0);return "Bridge hook relay failed "+JSON.stringify(context)+(fields.length?" "+fields.join(" "):" code=UNKNOWN")};',
    'let d="";',
    'process.stdin.setEncoding("utf8");',
    'process.stdin.on("data",c=>d+=c);',
    'process.stdin.on("end",async()=>{try{',
    `const k=process.env[${JSON.stringify(HOOK_RELAY_TOKEN_ENV)}];`,
    'if(!k){process.stderr.write("Missing hook relay credential");process.exitCode=1;return}',
    'const {request}=require("node:http");',
    `const r=await new Promise((resolve,reject)=>{const q=request(${JSON.stringify(url)},`,
    '{method:"POST",agent:false,headers:{"Content-Type":"application/json","Authorization":"Bearer "+k,"X-Bridge-Hook-Relay":"command"}},',
    's=>{let t="";s.setEncoding("utf8");s.on("data",c=>t+=c);s.on("error",reject);',
    's.on("end",()=>resolve({status:s.statusCode,text:t}))});q.on("error",reject);q.end(d)});',
    'const t=r.text;',
    'if(r.status<200||r.status>=300){process.stderr.write(diagnostic({code:"HTTP_"+r.status}));process.exitCode=1;return}',
    'let j;try{j=JSON.parse(t)}catch{process.stderr.write(diagnostic({code:"INVALID_RESPONSE"}));process.exitCode=1;return}',
    'const o=j&&j.result;',
    'if(!o||typeof o!=="object"){process.stderr.write(diagnostic({code:"MISSING_RESULT"}));process.exitCode=1;return}',
    'if(typeof o.stdout==="string")process.stdout.write(o.stdout);',
    'if(typeof o.stderr==="string")process.stderr.write(o.stderr);',
    'const x=Number(o.exitCode);process.exitCode=Number.isInteger(x)?x:1;',
    '}catch(e){process.stderr.write(diagnostic(e));process.exitCode=1}});',
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
): HookSettings {
  const annotated = registerSessionHooks(sessionId, hooks, sourceFn)
  const out: HookSettings = {}
  const proxyOrigin = `http://127.0.0.1:${config.port}`

  for (const [event, matchers] of Object.entries(hooks)) {
    if (!Array.isArray(matchers)) continue
    const newMatchers = matchers.map((m) => {
      if (!m || !Array.isArray(m.hooks)) return m
      const newHandlers: HookHandler[] = m.hooks.map((h) => {
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
          args: ['-e', buildHookCommandRelay(proxyUrl, { event, type: h.type, host: reg.effectiveHost })],
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
