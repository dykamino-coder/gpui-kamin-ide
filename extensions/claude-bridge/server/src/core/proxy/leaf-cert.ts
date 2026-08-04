// ============================================================================
// On-the-fly leaf certificate minting for MITM. Per-host LRU cache.
// Uses @peculiar/x509 (WebCrypto, RFC-5280 compliant) — node-forge omits
// AuthorityKeyIdentifier and Node 22's stricter TLS validator silently
// rejects such leaves with ECONNRESET on the client handshake.
// ============================================================================

import * as x509 from "@peculiar/x509"
import * as asn1X509 from "@peculiar/asn1-x509"
import { AsnConvert } from "@peculiar/asn1-schema"
import { webcrypto } from "node:crypto"
import type { BridgeCa } from "./ca-store"

interface LeafCert {
  certPem: string
  keyPem: string
  expiresAt: number
}

const CACHE_LIMIT = 256
const CERT_TTL_MS = 30 * 24 * 60 * 60 * 1000 // 30 days
const cache = new Map<string, LeafCert>()

const KEY_ALGO: RsaHashedKeyGenParams = {
  name: "RSASSA-PKCS1-v1_5",
  modulusLength: 2048,
  publicExponent: new Uint8Array([1, 0, 1]),
  hash: "SHA-256",
}

// One leaf keypair shared across all hosts — saves ~150ms per first-touch.
// Per-leaf signature still binds the cert to the specific SAN.
let leafKeyPairPromise: Promise<CryptoKeyPair> | null = null
async function getLeafKeyPair(): Promise<CryptoKeyPair> {
  if (!leafKeyPairPromise) {
    leafKeyPairPromise = webcrypto.subtle.generateKey(KEY_ALGO, true, ["sign", "verify"]) as Promise<CryptoKeyPair>
  }
  return leafKeyPairPromise
}

let caState: { cert: x509.X509Certificate; signingKey: CryptoKey } | null = null
async function loadCAOnce(ca: BridgeCa): Promise<{ cert: x509.X509Certificate; signingKey: CryptoKey }> {
  if (caState) return caState
  // Provide WebCrypto provider so peculiar can sign + parse PEMs.
  ;(x509 as any).cryptoProvider.set(webcrypto as any)
  const caCert = new x509.X509Certificate(ca.certPem)
  // Import PKCS#8 private key from PEM
  const keyB64 = ca.keyPem
    .replace(/-----BEGIN [^-]+-----/g, "")
    .replace(/-----END [^-]+-----/g, "")
    .replace(/\s+/g, "")
  const keyDer = Buffer.from(keyB64, "base64")
  const signingKey = await webcrypto.subtle.importKey(
    "pkcs8",
    keyDer,
    KEY_ALGO,
    false,
    ["sign"],
  )
  caState = { cert: caCert, signingKey }
  return caState
}

function pemFromArrayBuffer(buf: ArrayBuffer, label: string): string {
  const b64 = Buffer.from(buf).toString("base64")
  const lines: string[] = []
  for (let i = 0; i < b64.length; i += 64) lines.push(b64.slice(i, i + 64))
  return `-----BEGIN ${label}-----\n${lines.join("\n")}\n-----END ${label}-----\n`
}

function generateSerial(): string {
  const buf = new Uint8Array(20)
  webcrypto.getRandomValues(buf)
  buf[0] = buf[0]! & 0x7f // ensure positive
  return Array.from(buf).map((b) => b.toString(16).padStart(2, "0")).join("")
}

const inflight = new Map<string, Promise<{ certPem: string; keyPem: string }>>()

export async function mintLeafCert(host: string, ca: BridgeCa): Promise<{ certPem: string; keyPem: string }> {
  const hit = cache.get(host)
  if (hit && hit.expiresAt > Date.now()) return { certPem: hit.certPem, keyPem: hit.keyPem }

  const flying = inflight.get(host)
  if (flying) return flying

  const p = (async () => {
    try {
      const { cert: caCert, signingKey: caKey } = await loadCAOnce(ca)
      const leafKp = await getLeafKeyPair()

      const subjectDN = new x509.Name([
        { CN: [host] },
        { O: ["open-claude-bridge MITM"] },
      ]).toString()
      const issuerDN = caCert.subject

      const notBefore = new Date(Date.now() - 24 * 60 * 60 * 1000)
      const notAfter = new Date(Date.now() + CERT_TTL_MS)

      const extensions: x509.Extension[] = []
      extensions.push(new x509.BasicConstraintsExtension(false, undefined, true))
      extensions.push(new x509.KeyUsagesExtension(
        x509.KeyUsageFlags.digitalSignature | x509.KeyUsageFlags.keyEncipherment,
        true,
      ))
      extensions.push(new x509.ExtendedKeyUsageExtension(
        [asn1X509.id_kp_serverAuth, asn1X509.id_kp_clientAuth],
        false,
      ))
      extensions.push(new x509.SubjectAlternativeNameExtension(
        [{ type: "dns", value: host }],
        false,
      ))
      const policyInfo = new asn1X509.PolicyInformation({ policyIdentifier: "2.23.140.1.2.1" })
      const policies = new asn1X509.CertificatePolicies([policyInfo])
      extensions.push(new x509.Extension(
        asn1X509.id_ce_certificatePolicies,
        false,
        AsnConvert.serialize(policies),
      ))
      extensions.push(await x509.AuthorityKeyIdentifierExtension.create(caCert, false))

      const cert = await x509.X509CertificateGenerator.create({
        serialNumber: generateSerial(),
        subject: subjectDN,
        issuer: issuerDN,
        notBefore,
        notAfter,
        signingAlgorithm: KEY_ALGO,
        publicKey: leafKp.publicKey,
        signingKey: caKey,
        extensions,
      })

      const certPem = cert.toString("pem")
      const keyDer = await webcrypto.subtle.exportKey("pkcs8", leafKp.privateKey)
      const keyPem = pemFromArrayBuffer(keyDer, "PRIVATE KEY")

      const result = { certPem, keyPem }
      cache.set(host, { certPem, keyPem, expiresAt: Date.now() + CERT_TTL_MS })
      if (cache.size > CACHE_LIMIT) {
        const first = cache.keys().next().value
        if (first !== undefined) cache.delete(first)
      }
      return result
    } finally {
      inflight.delete(host)
    }
  })()
  inflight.set(host, p)
  return p
}
