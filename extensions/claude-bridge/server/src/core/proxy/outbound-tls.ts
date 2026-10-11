// Server outbound TLS policy and content-addressed CLI trust bundles.
// Keep this process's policy separate from child CLI certificate verification.
import { createHash, X509Certificate } from 'node:crypto'
import * as fs from 'node:fs'
import * as https from 'node:https'
import * as path from 'node:path'
import * as tls from 'node:tls'
import { EnvHttpProxyAgent, ProxyAgent, setGlobalDispatcher } from 'undici'
import type { Dispatcher } from 'undici'

export const INSECURE_TLS_WARNING =
  '[bridge] WARNING: outbound HTTPS certificate verification is DISABLED. Anyone on the network path can intercept tokens, prompts and transcripts. Set BRIDGE_TLS_VERIFY=1 and configure trusted CA roots to enable verification.'

export function tlsVerificationEnabled(value = process.env.BRIDGE_TLS_VERIFY): boolean {
  if (value === '1') return true
  if (value === undefined || value === '' || value === '0') return false
  throw new Error('BRIDGE_TLS_VERIFY must be 0 or 1')
}

/** Explicit ca replaces Node defaults, so retain all default AND system roots. */
export function assembleCaBundle(...sources: readonly string[][]): string[] {
  return [
    ...new Set(
      sources
        .flat()
        .map((cert) => cert.trim())
        .filter(Boolean),
    ),
  ]
}

export function readCaFile(filename: string): string[] {
  const pem = fs.readFileSync(filename, 'utf8')
  const certs = pem.match(/-----BEGIN CERTIFICATE-----[\s\S]*?-----END CERTIFICATE-----/g)
  if (!certs?.length) throw new Error(`CA file contains no PEM certificates: ${filename}`)
  // A configured but invalid root must fail visibly, never silently drop trust.
  for (const cert of certs) new X509Certificate(cert)
  return certs
}

export function trustedRoots(extraCaPath?: string | null): string[] {
  return assembleCaBundle(
    tls.getCACertificates('default'),
    tls.getCACertificates('system'),
    extraCaPath ? readCaFile(extraCaPath) : [],
  )
}

export function outboundTlsOptions(extraCaPath?: string | null): tls.ConnectionOptions {
  return { rejectUnauthorized: tlsVerificationEnabled(), ca: trustedRoots(extraCaPath) }
}

export function createOutboundDispatcher(extraCaPath?: string | null, proxyUrl?: string): Dispatcher {
  const options = outboundTlsOptions(extraCaPath)
  // requestTls verifies the inspected upstream, proxyTls the HTTPS CONNECT proxy.
  const agentOptions = { connect: options, requestTls: options, proxyTls: options }
  return proxyUrl ? new ProxyAgent({ uri: proxyUrl, ...agentOptions }) : new EnvHttpProxyAgent(agentOptions)
}

let globalDispatcher: Dispatcher | undefined
let startupWarningEmitted = false

export function initializeOutboundTls(extraCaPath?: string | null): void {
  configureOutboundTls(extraCaPath)
  if (!tlsVerificationEnabled() && !startupWarningEmitted) {
    console.warn(INSECURE_TLS_WARNING)
    startupWarningEmitted = true
  }
}

/** Covers native/global fetch and node:https consumers (including HTTPS hooks). */
export function configureOutboundTls(extraCaPath?: string | null): void {
  const options = outboundTlsOptions(extraCaPath)
  const next = createOutboundDispatcher(extraCaPath)
  const previous = globalDispatcher
  setGlobalDispatcher(next)
  globalDispatcher = next
  Object.assign(https.globalAgent.options, options)
  // Do not reuse sockets authenticated under the previous trust configuration.
  https.globalAgent.destroy()
  if (previous) void previous.close().catch(() => {})
}

/** NODE_EXTRA_CA_CERTS accepts one file; avoid overwriting the operator's roots.
 * Content-addressing prevents concurrent sessions/CA changes rewriting a live bundle. */
export function buildCliCaBundle(bridgeCaPath: string, extraCaPath?: string): string {
  const roots = assembleCaBundle(trustedRoots(extraCaPath), readCaFile(bridgeCaPath))
  const pem = roots.join('\n') + '\n'
  const hash = createHash('sha256').update(pem).digest('hex')
  const filename = path.join(path.dirname(bridgeCaPath), `cli-ca-${hash}.pem`)
  try {
    fs.writeFileSync(filename, pem, { mode: 0o644, flag: 'wx' })
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error
  }
  return filename
}
