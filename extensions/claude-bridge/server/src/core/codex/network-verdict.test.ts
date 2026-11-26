import { describe, expect, it } from 'vitest'
import { summarizeVerdicts, verdictFromAnswer, verdictFromError, type HostVerdict } from './network-verdict'

// Answers as the field saw them, without any network.
const SQUID_PAGE =
  '<html><head><title>ERROR: The requested URL could not be retrieved</title></head><body>Access Denied</body></html>'
const REGION_BODY = JSON.stringify({
  error: { code: 'unsupported_country_region_territory', message: 'Country, region, or territory not supported' },
})

describe('verdictFromAnswer', () => {
  it('names a 403 page from the proxy, not from OpenAI', () => {
    const v = verdictFromAnswer('auth.openai.com', {
      status: 403,
      headers: { server: 'squid/6.10', 'x-squid-error': 'ERR_ACCESS_DENIED 0', 'content-type': 'text/html' },
      body: SQUID_PAGE,
    })
    expect(v.kind).toBe('proxy-denied')
    expect(v.detail).toContain('without cf-ray')
    expect(v.detail).toContain('server: squid/6.10')
    expect(v.detail).toContain('ERR_ACCESS_DENIED')
  })

  it("names OpenAI's region refusal by its error code", () => {
    const v = verdictFromAnswer('api.openai.com', {
      status: 403,
      headers: { 'cf-ray': '8f00000000000000-FRA', server: 'cloudflare' },
      body: REGION_BODY,
    })
    expect(v).toMatchObject({ kind: 'region-blocked', status: 403 })
  })

  it('accepts any answer that carries the target edge markers', () => {
    expect(verdictFromAnswer('chatgpt.com', { status: 200, headers: { 'cf-ray': 'x' }, body: '' }).kind).toBe('ok')
    // api.openai.com without a key: 401 from OpenAI is the expected answer.
    expect(
      verdictFromAnswer('api.openai.com', { status: 401, headers: { 'openai-processing-ms': '3' }, body: '{}' }).kind,
    ).toBe('ok')
    // A Cloudflare challenge is still OpenAI's edge, not a proxy.
    expect(verdictFromAnswer('chatgpt.com', { status: 403, headers: { server: 'cloudflare' }, body: '' }).kind).toBe(
      'ok',
    )
  })

  it('names a 407 page as proxy authentication', () => {
    expect(verdictFromAnswer('chatgpt.com', { status: 407, headers: {}, body: '' }).kind).toBe('proxy-auth')
  })
})

describe('verdictFromError', () => {
  it('names a refused CONNECT as the proxy refusing the host', () => {
    const v = verdictFromError('auth.openai.com', 'Proxy response (403) !== 200 when HTTP Tunneling')
    expect(v).toMatchObject({ kind: 'proxy-denied', status: 403 })
    expect(v.detail).toContain('CONNECT auth.openai.com:443')
    expect(verdictFromError('auth.openai.com', 'Proxy response (407) !== 200 when HTTP Tunneling').kind).toBe(
      'proxy-auth',
    )
  })

  it('names TLS, connection and DNS failures', () => {
    expect(
      verdictFromError('chatgpt.com', 'unable to get local issuer certificate ← UNABLE_TO_GET_ISSUER_CERT_LOCALLY')
        .kind,
    ).toBe('tls')
    expect(verdictFromError('chatgpt.com', 'getaddrinfo ENOTFOUND chatgpt.com ← ENOTFOUND').kind).toBe('network')
    expect(verdictFromError('chatgpt.com', 'connect ECONNREFUSED 10.0.0.1:3128 ← ECONNREFUSED').kind).toBe('network')
    expect(verdictFromError('chatgpt.com', 'something odd').kind).toBe('other')
  })
})

describe('summarizeVerdicts', () => {
  const ok = (host: string): HostVerdict => ({ host, kind: 'ok', status: 200, detail: 'HTTP 200' })

  it('is null when every host answered', () => {
    expect(summarizeVerdicts([ok('auth.openai.com'), ok('chatgpt.com')], true)).toBeNull()
  })

  it('tells the user to allow the codex domains on the proxy, naming the refused hosts', () => {
    const denied: HostVerdict = {
      host: 'auth.openai.com',
      kind: 'proxy-denied',
      status: 403,
      detail: 'HTTP 403 without cf-ray',
    }
    const info = summarizeVerdicts([denied, ok('chatgpt.com'), ok('api.openai.com')], true)
    expect(info?.kind).toBe('proxy-denied')
    expect(info?.message).toContain('The proxy refuses auth.openai.com')
    expect(info?.message).toContain('auth.openai.com, chatgpt.com, api.openai.com')
    expect(info?.detail).toBe('auth.openai.com: HTTP 403 without cf-ray')
  })

  it('says OpenAI refuses the exit region, not the proxy', () => {
    const region: HostVerdict = { host: 'api.openai.com', kind: 'region-blocked', status: 403, detail: 'HTTP 403' }
    const info = summarizeVerdicts([ok('auth.openai.com'), region], false)
    expect(info?.kind).toBe('region-blocked')
    expect(info?.message).toMatch(/OpenAI does not accept this network's exit address/)
  })

  it('puts the missing CA before the refusals it may hide, and says where the CA goes', () => {
    const tls: HostVerdict = { host: 'chatgpt.com', kind: 'tls', status: null, detail: 'self-signed certificate' }
    const denied: HostVerdict = { host: 'auth.openai.com', kind: 'proxy-denied', status: 403, detail: 'x' }
    const info = summarizeVerdicts([denied, tls], true)
    expect(info?.kind).toBe('tls')
    expect(info?.message).toContain('Network / Proxy → CA Certificate')
    expect(info?.detail).toContain('auth.openai.com: x; chatgpt.com: self-signed certificate')
  })

  it('names a connection failure with every unreachable host', () => {
    const down = (host: string): HostVerdict => ({ host, kind: 'network', status: null, detail: 'ENOTFOUND' })
    const info = summarizeVerdicts([down('auth.openai.com'), down('chatgpt.com')], false)
    expect(info?.message).toContain('Bridge cannot connect to auth.openai.com, chatgpt.com')
  })

  it('names nothing when the only failures are unrecognised', () => {
    expect(summarizeVerdicts([{ host: 'chatgpt.com', kind: 'other', status: null, detail: '?' }], false)).toBeNull()
  })
})
