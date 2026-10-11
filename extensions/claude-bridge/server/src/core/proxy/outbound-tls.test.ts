// CA assembly and separation of server policy from child CLI trust.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'
import * as https from 'node:https'
import * as tls from 'node:tls'
import { EnvHttpProxyAgent, ProxyAgent, setGlobalDispatcher } from 'undici'
import {
  assembleCaBundle,
  buildCliCaBundle,
  configureOutboundTls,
  createOutboundDispatcher,
  initializeOutboundTls,
  INSECURE_TLS_WARNING,
  outboundTlsOptions,
  readCaFile,
  tlsVerificationEnabled,
  trustedRoots,
} from './outbound-tls'

vi.mock('node:tls', async (importOriginal) => {
  const actual = await importOriginal<typeof import('node:tls')>()
  return { ...actual, getCACertificates: vi.fn((type: string) => [actual.rootCertificates[type === 'system' ? 1 : 0]]) }
})
vi.mock('undici', () => ({
  EnvHttpProxyAgent: vi.fn(function () {
    return { close: vi.fn(async () => {}) }
  }),
  ProxyAgent: vi.fn(function () {
    return { close: vi.fn(async () => {}) }
  }),
  setGlobalDispatcher: vi.fn(),
}))

let dir: string
let bridge: string
let corporate: string
const originalAgentOptions = { ...https.globalAgent.options }

beforeEach(() => {
  vi.clearAllMocks()
  vi.stubEnv('BRIDGE_TLS_VERIFY', '')
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-ca-unit-'))
  bridge = path.join(dir, 'bridge.pem')
  corporate = path.join(dir, 'corporate.pem')
  // Public bundled roots stand in for two independent, valid PEM certificates.
  fs.writeFileSync(bridge, tls.rootCertificates[2]!)
  fs.writeFileSync(corporate, tls.rootCertificates[3]!)
})
afterEach(() => {
  vi.unstubAllEnvs()
  vi.restoreAllMocks()
  delete https.globalAgent.options.ca
  delete https.globalAgent.options.rejectUnauthorized
  Object.assign(https.globalAgent.options, originalAgentOptions)
  fs.rmSync(dir, { recursive: true, force: true })
})

describe('outbound TLS policy', () => {
  it('defaults to unverified TLS and requires an explicit valid opt-in', () => {
    expect(tlsVerificationEnabled(undefined)).toBe(false)
    expect(tlsVerificationEnabled('0')).toBe(false)
    expect(outboundTlsOptions().rejectUnauthorized).toBe(false)
    vi.stubEnv('BRIDGE_TLS_VERIFY', '1')
    expect(outboundTlsOptions().rejectUnauthorized).toBe(true)
    expect(() => tlsVerificationEnabled('true')).toThrow('must be 0 or 1')
  })

  it.each(['', '1'])('applies mode %s to direct, upstream and HTTPS proxy TLS', (mode) => {
    vi.stubEnv('BRIDGE_TLS_VERIFY', mode)
    createOutboundDispatcher(corporate)
    createOutboundDispatcher(corporate, 'https://proxy.example:8443')
    const options = { rejectUnauthorized: mode === '1', ca: trustedRoots(corporate) }
    expect(EnvHttpProxyAgent).toHaveBeenCalledWith({
      connect: options,
      requestTls: options,
      proxyTls: options,
    })
    expect(ProxyAgent).toHaveBeenCalledWith({
      uri: 'https://proxy.example:8443',
      connect: options,
      requestTls: options,
      proxyTls: options,
    })
    configureOutboundTls(corporate)
    expect(setGlobalDispatcher).toHaveBeenCalled()
    expect(https.globalAgent.options).toMatchObject(options)
    expect(process.env.NODE_TLS_REJECT_UNAUTHORIZED).toBeUndefined()
  })

  it('logs one unfiltered warning at startup when verification is off', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    vi.stubEnv('BRIDGE_TLS_VERIFY', '1')
    initializeOutboundTls()
    expect(warn).not.toHaveBeenCalled()
    vi.stubEnv('BRIDGE_TLS_VERIFY', '')
    initializeOutboundTls()
    initializeOutboundTls()
    expect(warn).toHaveBeenCalledExactlyOnceWith(INSECURE_TLS_WARNING)
  })
})

describe('CA assembly', () => {
  it('retains bundled and system roots while adding an operator PEM bundle', () => {
    fs.appendFileSync(corporate, '\n' + tls.rootCertificates[4]!)
    expect(trustedRoots(corporate)).toEqual([
      tls.rootCertificates[0]!.trim(),
      tls.rootCertificates[1]!.trim(),
      tls.rootCertificates[3]!.trim(),
      tls.rootCertificates[4]!.trim(),
    ])
    expect(assembleCaBundle([' A ', 'B'], ['A', '', 'C'])).toEqual(['A', 'B', 'C'])
  })

  it('fails visibly for missing, empty and malformed configured certificates', () => {
    expect(() => trustedRoots(path.join(dir, 'missing'))).toThrow()
    fs.writeFileSync(corporate, 'not a certificate')
    expect(() => readCaFile(corporate)).toThrow('no PEM certificates')
    fs.writeFileSync(corporate, '-----BEGIN CERTIFICATE-----\ninvalid\n-----END CERTIFICATE-----')
    expect(() => trustedRoots(corporate)).toThrow()
  })

  it.each(['', '1'])('preserves Bridge AND corporate CLI trust in server mode %s', (mode) => {
    vi.stubEnv('BRIDGE_TLS_VERIFY', mode)
    const filename = buildCliCaBundle(bridge, corporate)
    const certs = readCaFile(filename)
    expect(certs).toEqual([...trustedRoots(corporate), tls.rootCertificates[2]!.trim()])
    expect(buildCliCaBundle(bridge, corporate)).toBe(filename)
    fs.writeFileSync(corporate, tls.rootCertificates[4]!)
    const rotated = buildCliCaBundle(bridge, corporate)
    expect(rotated).not.toBe(filename)
    expect(readCaFile(filename)).toEqual(certs)
    expect(readCaFile(rotated)).toContain(tls.rootCertificates[4]!.trim())
    expect(readCaFile(buildCliCaBundle(bridge))).toContain(tls.rootCertificates[2]!.trim())
  })
})
