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
  origin = https.createServer({ cert: leaf.certPem, key: leaf.keyPem }, (_req, res) => res.end('ok'))
  const originPort = await listen(origin)
  proxy = http.createServer((_req, res) => res.writeHead(405).end())
  proxy.on('connect', (req, socket: net.Socket, head) => {
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

const url = `https://${HOST}/`

describe('codex network probe', () => {
  it('names a proxy that demands credentials (HTTP 407)', async () => {
    const info = await probeCodexNetwork({ HTTPS_PROXY: `http://127.0.0.1:${proxyPort}` }, { url })
    expect(info?.kind).toBe('proxy-auth')
    expect(info?.detail).toMatch(/407/)
  })

  it('names a TLS-intercepting proxy whose CA is not trusted', async () => {
    const info = await probeCodexNetwork({ HTTPS_PROXY: `http://acc:pw@127.0.0.1:${proxyPort}` }, { url })
    expect(info?.kind).toBe('tls')
  })

  it('passes once the corporate CA is given to codex', async () => {
    const env = { HTTPS_PROXY: `http://acc:pw@127.0.0.1:${proxyPort}`, CODEX_CA_CERTIFICATE: caFile }
    expect(await probeCodexNetwork(env, { url })).toBeNull()
  })

  it('names an unreachable proxy as a network problem', async () => {
    const closed = net.createServer()
    const port = await listen(closed)
    await new Promise((r) => closed.close(r))
    const info = await probeCodexNetwork({ HTTPS_PROXY: `http://127.0.0.1:${port}` }, { url })
    expect(info?.kind).toBe('network')
  })
})
