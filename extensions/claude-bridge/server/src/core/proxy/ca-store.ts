// ============================================================================
// MITM CA store — generate self-signed CA on first run, cache on disk.
// CLI processes spawned by the bridge trust this CA via NODE_EXTRA_CA_CERTS.
// Nothing is added to the system trust store.
//
// Uses @peculiar/x509 (RFC-5280 compliant via WebCrypto) — same library as
// leaf-cert.ts. Earlier we relied on mockttp.generateCACertificate, but the
// rest of mockttp is no longer used and dragging it in just for one helper
// added ~600KB to the install footprint.
// ============================================================================

import { webcrypto } from "node:crypto"
import * as fs from "node:fs"
import * as os from "node:os"
import * as path from "node:path"
import * as x509 from "@peculiar/x509"

export interface BridgeCa {
  certPem: string
  keyPem: string
  certPath: string
  keyPath: string
}

let cached: BridgeCa | null = null
let initPromise: Promise<BridgeCa> | null = null

function caDir(): string {
  return path.join(os.homedir(), ".claude", "open-claude-bridge")
}

const KEY_ALGO: RsaHashedKeyGenParams = {
  name: "RSASSA-PKCS1-v1_5",
  modulusLength: 2048,
  publicExponent: new Uint8Array([1, 0, 1]),
  hash: "SHA-256",
}
const SIGN_ALGO: EcdsaParams | RsaPssParams | { name: string; hash: string } = {
  name: "RSASSA-PKCS1-v1_5",
  hash: "SHA-256",
}

async function generateCa(): Promise<{ certPem: string; keyPem: string }> {
  ;(x509 as unknown as { cryptoProvider: { set: (p: unknown) => void } })
    .cryptoProvider.set(webcrypto as unknown as object)

  const keyPair = (await webcrypto.subtle.generateKey(KEY_ALGO, true, ["sign", "verify"])) as CryptoKeyPair

  const serial = new Uint8Array(16)
  webcrypto.getRandomValues(serial)
  const notBefore = new Date()
  const notAfter = new Date(notBefore.getTime() + 10 * 365 * 24 * 60 * 60 * 1000)

  const cert = await x509.X509CertificateGenerator.createSelfSigned({
    serialNumber: Buffer.from(serial).toString("hex"),
    name: "CN=open-claude-bridge MITM, O=open-claude-bridge",
    notBefore,
    notAfter,
    signingAlgorithm: SIGN_ALGO,
    keys: keyPair,
    extensions: [
      new x509.BasicConstraintsExtension(true, undefined, true),
      new x509.KeyUsagesExtension(
        x509.KeyUsageFlags.keyCertSign
          | x509.KeyUsageFlags.cRLSign
          | x509.KeyUsageFlags.digitalSignature,
        true,
      ),
      await x509.SubjectKeyIdentifierExtension.create(keyPair.publicKey),
    ],
  })

  const certPem = cert.toString("pem")
  const pkcs8 = await webcrypto.subtle.exportKey("pkcs8", keyPair.privateKey)
  const keyB64 = Buffer.from(pkcs8).toString("base64")
  const keyPem = `-----BEGIN PRIVATE KEY-----\n${keyB64.match(/.{1,64}/g)?.join("\n") ?? keyB64}\n-----END PRIVATE KEY-----\n`
  return { certPem, keyPem }
}

export async function getOrCreateBridgeCA(): Promise<BridgeCa> {
  if (cached) return cached
  if (initPromise) return initPromise
  initPromise = (async () => {
    const dir = caDir()
    fs.mkdirSync(dir, { recursive: true })
    const certPath = path.join(dir, "proxy-ca.pem")
    const keyPath = path.join(dir, "proxy-ca.key")
    if (fs.existsSync(certPath) && fs.existsSync(keyPath)) {
      const certPem = fs.readFileSync(certPath, "utf8")
      const keyPem = fs.readFileSync(keyPath, "utf8")
      cached = { certPem, keyPem, certPath, keyPath }
      return cached
    }
    const fresh = await generateCa()
    fs.writeFileSync(certPath, fresh.certPem, { mode: 0o644 })
    fs.writeFileSync(keyPath, fresh.keyPem, { mode: 0o600 })
    cached = { certPem: fresh.certPem, keyPem: fresh.keyPem, certPath, keyPath }
    return cached
  })()
  return initPromise
}

export function clearBridgeCAForTesting(): void {
  cached = null
  initPromise = null
}
