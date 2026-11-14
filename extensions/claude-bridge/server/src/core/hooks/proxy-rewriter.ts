// Take user's HookSettings → produce CLI-bound HookSettings where every
// local command hook points at our /api/hooks/<session>/<event>/<id>
// endpoint. Most events get an exec-form Node relay that reproduces the local
// command's stdout, stderr, and exit code inside the Linux CLI process, so
// Claude Code applies its own event-specific command-hook semantics (including
// exit 2). The per-tool events in HTTP_HOOK_EVENTS get a native http hook
// instead, answered with the body that has the same effect (http-hook-answer).
// Native argument conditions on command/HTTP hooks are evaluated by the
// dispatcher. Other handlers stay in the CLI, with bare tool names translated.

import { config } from '../config'
import type { HookHandler, HookMatcher, HookSettings, HookSource } from './types'
import { registerSessionHooks } from './registry'
import { matcherForCli } from './tool-names'
import { conditionForCli } from './hook-condition'
import { HTTP_HOOK_EVENTS, HTTP_HOOK_RELAY } from './http-hook-answer'
import { BASH_HOME_ENV, BASH_HOME_HEADER } from './hook-environment'
export { BASH_HOME_HEADER } from './hook-environment'

/** Env var the CLI process carries for us. The relay credential travels in
 *  the session's own process environment, never in the command text: the
 *  rewritten declaration is shown in Console, JSONL and the hooks panel, and a
 *  Bearer value interpolated there would be readable in all three. Hook
 *  children inherit the CLI's environment, so the relay still authenticates. */
export const HOOK_RELAY_TOKEN_ENV = 'CLAUDE_BRIDGE_HOOK_TOKEN'

export function buildHookCommandRelay(
  url: string,
  context = { event: 'unknown', type: 'command', host: 'local' },
  bashCondition = false,
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
    `{method:"POST",agent:false,headers:{"Content-Type":"application/json","Authorization":"Bearer "+k,"X-Bridge-Hook-Relay":"command"${bashCondition ? `,${JSON.stringify(BASH_HOME_HEADER)}:process.env[${JSON.stringify(BASH_HOME_ENV)}]||""` : ''}}},`,
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

/** Whether a relayed command hook goes to the CLI as a native http hook.
 *  Async hooks stay on the command relay: an http hook has no `asyncRewake`.
 *  `KAMIN_HOOK_HTTP=0` keeps every hook on the relay. */
function useHttpHook(event: string, h: HookHandler): boolean {
  if (process.env.KAMIN_HOOK_HTTP === '0') return false
  if (h.type !== 'command' || h.async || h.asyncRewake) return false
  return HTTP_HOOK_EVENTS.has(event)
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
        const condition = conditionForCli(event, h.if)
        const translated = condition.cliIf === h.if ? h : { ...h, if: condition.cliIf }
        // LLM hooks still execute inside the CLI. Bare names can be mapped;
        // argument conditions need a CLI-side predicate/LLM continuation.
        if (h.type !== 'command' && h.type !== 'http') return condition.serverRule ? h : translated
        if (condition.serverRule) {
          reg.serverCondition = condition.serverRule
        }
        // These declarations are emitted into --settings, where the CLI
        // ignores once. Only skill-frontmatter hooks have once semantics;
        // filtering settings hooks must not introduce a server-side limit.
        const once = h.once
        if (h.type === 'http' && !condition.serverRule) return translated
        if (h.type === 'command' && reg.effectiveHost === 'server' && !condition.serverRule) return translated

        const proxyUrl = `${proxyOrigin}/api/hooks/${sessionId}/${encodeURIComponent(event)}/${reg.id}`

        // Per-tool events go out as native http hooks: the CLI sends them
        // in-process instead of starting a Node relay on every tool call. The
        // header names the credential's env var, so its value is substituted
        // only in the request and never appears in the declaration.
        if (h.type === 'http' || useHttpHook(event, h)) {
          const http: HookHandler = {
            type: 'http',
            url: proxyUrl,
            headers: {
              Authorization: `Bearer $${HOOK_RELAY_TOKEN_ENV}`,
              'X-Bridge-Hook-Relay': HTTP_HOOK_RELAY,
            },
            allowedEnvVars: [HOOK_RELAY_TOKEN_ENV],
          }
          if (h.timeout !== undefined) http.timeout = h.timeout
          if (condition.serverRule?.tool === 'Bash') {
            http.headers![BASH_HOME_HEADER] = `$${BASH_HOME_ENV}`
            http.allowedEnvVars!.push(BASH_HOME_ENV)
          }
          if (condition.cliIf !== undefined) http.if = condition.cliIf
          if (h.statusMessage !== undefined) http.statusMessage = h.statusMessage
          if (once !== undefined) http.once = once
          return http
        }

        // Keep command-hook semantics on both sides of the bridge. Exec-form
        // avoids quoting/token expansion in the container and works for sync,
        // async, and asyncRewake without event-specific HTTP translations.
        const relay: HookHandler = {
          type: 'command',
          command: 'node',
          args: [
            '-e',
            buildHookCommandRelay(
              proxyUrl,
              { event, type: h.type, host: reg.effectiveHost },
              condition.serverRule?.tool === 'Bash',
            ),
          ],
          timeout: h.timeout,
          if: condition.cliIf,
          statusMessage: h.statusMessage,
          once,
        }
        if (h.async || h.asyncRewake) {
          relay.async = true
          relay.asyncRewake = h.asyncRewake
        }
        return relay
      })
      return { matcher: matcherForCli(event, m.matcher), hooks: newHandlers }
    })
    ;(out as Record<string, unknown>)[event] = newMatchers
  }
  return out
}
