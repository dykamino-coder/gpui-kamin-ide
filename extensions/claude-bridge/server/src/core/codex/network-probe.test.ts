import 'reflect-metadata'
import fs from 'node:fs'
import http from 'node:http'
import net from 'node:net'
import os from 'node:os'
import path from 'node:path'
import https from 'node:https'
import { webcrypto } from 'node:crypto'
import * as x509 from '@peculiar/x509'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { mintLeafCert } from '../proxy/leaf-cert'
import { probeCodexNetwork } from './network-probe'

// A corporate network in miniature: a CONNECT proxy (optionally demanding
// basic auth) in front of an HTTPS server whose certificate comes from a
// private CA — what a TLS-intercepting proxy presents.
const HOST = 'chatgpt.probe.test'
/** A host whose CONNECT the proxy refuses outright. */
const BLOCKED = 'blocked.probe.test'
const AUTH = `Basic ${Buffer.from('acc:pw').toString('base64')}`

async function privateCa(): Promise<{ certPem: string; keyPem: string }> {
  ;(x509 as unknown as { cryptoProvider: { set: (p: unknown) => void } }).cryptoProvider.set(webcrypto)
  const alg = {
    name: 'RSASSA-PKCS1-v1_5',
    modulusLength: 2048,
    publicExponent: new Uint8Array([1, 0, 1]),
    hash: 'SHA-256',
  }
  const keys = (await webcrypto.subtle.generateKey(alg, true, ['sign', 'verify'])) as CryptoKeyPair
  const cert = await x509.X509CertificateGenerator.createSelfSigned({
    serialNumber: '01',
    name: 'CN=Corp Intercept CA',
    notBefore: new Date(Date.now() - 86_400_000),
    notAfter: new Date(Date.now() + 86_400_000),
    signingAlgorithm: alg,
    keys,
    extensions: [
      new x509.BasicConstraintsExtension(true, undefined, true),
      new x509.KeyUsagesExtension(x509.KeyUsageFlags.keyCertSign | x509.KeyUsageFlags.digitalSignature, true),
    ],
  })
  const pkcs8 = Buffer.from(await webcrypto.subtle.exportKey('pkcs8', keys.privateKey)).toString('base64')
  return { certPem: cert.toString('pem'), keyPem: `-----BEGIN PRIVATE KEY-----\n${pkcs8}\n-----END PRIVATE KEY-----\n` }
}

let dir: string
let caFile: string
let origin: https.Server
let proxy: http.Server
let proxyPort: number

function listen(server: net.Server): Promise<number> {
  return new Promise((resolve) =>
    server.listen(0, '127.0.0.1', () => resolve((server.address() as net.AddressInfo).port)),
  )
}

beforeAll(async () => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'codex-net-probe-'))
  const ca = await privateCa()
  caFile = path.join(dir, 'corp-ca.pem')
  fs.writeFileSync(caFile, ca.certPem)
  const leaf = await mintLeafCert(HOST, { ...ca, certPath: '', keyPath: '' })
  // Paths stand in for the hosts: what OpenAI answers vs. what a
  // TLS-intercepting proxy answers inside the tunnel for a domain it blocks.
  origin = https.createServer({ cert: leaf.certPem, key: leaf.keyPem }, (req, res) => {
    if (req.url === '/bumped-denied') {
      res.writeHead(403, { server: 'squid', 'x-squid-error': 'ERR_ACCESS_DENIED 0', 'content-type': 'text/html' })
      return res.end('<html><title>ERROR: The requested URL could not be retrieved</title></html>')
    }
    if (req.url === '/region') {
      res.writeHead(403, { 'cf-ray': 'test-ray', 'content-type': 'application/json' })
      return res.end('{"error":{"code":"unsupported_country_region_territory"}}')
    }
    res.writeHead(200, { 'cf-ray': 'test-ray' }).end('ok')
  })
  const originPort = await listen(origin)
  proxy = http.createServer((_req, res) => res.writeHead(405).end())
  proxy.on('connect', (req, socket: net.Socket, head) => {
    if (req.url?.startsWith(`${BLOCKED}:`)) {
      socket.end('HTTP/1.1 403 Forbidden\r\nServer: squid\r\n\r\n')
      return
    }
    if (req.headers['proxy-authorization'] !== AUTH) {
      socket.end('HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic realm="corp"\r\n\r\n')
      return
    }
    const upstream = net.connect(originPort, '127.0.0.1', () => {
      socket.write('HTTP/1.1 200 Connection Established\r\n\r\n')
      upstream.write(head)
      upstream.pipe(socket).pipe(upstream)
    })
    upstream.on('error', () => socket.destroy())
    socket.on('error', () => upstream.destroy())
  })
  proxyPort = await listen(proxy)
}, 30_000)

afterAll(async () => {
  proxy?.closeAllConnections()
  origin?.closeAllConnections()
  await Promise.all([proxy, origin].map((s) => new Promise((r) => (s ? s.close(r) : r(null)))))
  fs.rmSync(dir, { recursive: true, force: true })
})

// Every probed host goes to the local origin; the path picks its answer.
const one = { hosts: ['chatgpt.com'], urlFor: () => `https://${HOST}/` }
const trusted = () => ({ HTTPS_PROXY: `http://acc:pw@127.0.0.1:${proxyPort}`, CODEX_CA_CERTIFICATE: caFile })

describe('codex network probe', () => {
  it('names a proxy that demands credentials (HTTP 407)', async () => {
    const report = await probeCodexNetwork({ HTTPS_PROXY: `http://127.0.0.1:${proxyPort}` }, one)
    expect(report.viaProxy).toBe(true)
    expect(report.problem?.kind).toBe('proxy-auth')
    expect(report.problem?.detail).toMatch(/407/)
  })

  it('names a TLS-intercepting proxy whose CA is not trusted', async () => {
    const report = await probeCodexNetwork({ HTTPS_PROXY: `http://acc:pw@127.0.0.1:${proxyPort}` }, one)
    expect(report.customCa).toBe(false)
    expect(report.problem?.kind).toBe('tls')
    expect(report.problem?.message).toContain('CA Certificate')
  })

  it('passes once the corporate CA is given to codex', async () => {
    const report = await probeCodexNetwork(trusted(), one)
    expect(report.problem).toBeNull()
    expect(report.customCa).toBe(true)
    expect(report.hosts).toEqual([
      { host: 'chatgpt.com', kind: 'ok', status: 200, detail: 'HTTP 200 from chatgpt.com' },
    ])
  })

  it('tells a refusing proxy from OpenAI refusing the region, host by host', async () => {
    const paths: Record<string, string> = {
      'auth.openai.com': 'bumped-denied',
      'chatgpt.com': '',
      'api.openai.com': 'region',
    }
    const report = await probeCodexNetwork(trusted(), { urlFor: (host) => `https://${HOST}/${paths[host]}` })
    expect(report.hosts.map((h) => [h.host, h.kind])).toEqual([
      ['auth.openai.com', 'proxy-denied'],
      ['chatgpt.com', 'ok'],
      ['api.openai.com', 'region-blocked'],
    ])
    expect(report.problem?.kind).toBe('proxy-denied')
    expect(report.problem?.message).toContain('The proxy refuses auth.openai.com')
    expect(report.problem?.detail).toContain('server: squid')
    expect(report.problem?.detail).toContain('api.openai.com: HTTP 403 unsupported_country_region_territory')
  })

  it('names a proxy that refuses the CONNECT to a host', async () => {
    const report = await probeCodexNetwork(trusted(), {
      hosts: ['auth.openai.com'],
      urlFor: () => `https://${BLOCKED}/`,
    })
    expect(report.hosts[0]).toMatchObject({ kind: 'proxy-denied', status: 403 })
    expect(report.problem?.message).toContain("proxy's allowed domains")
  })

  it('names an unreachable proxy as a network problem', async () => {
    const closed = net.createServer()
    const port = await listen(closed)
    await new Promise((r) => closed.close(r))
    const report = await probeCodexNetwork({ HTTPS_PROXY: `http://127.0.0.1:${port}` }, one)
    expect(report.problem?.kind).toBe('network')
  })
})
