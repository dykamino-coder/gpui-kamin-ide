// DPAPI relay for the extension `secrets` store (B9 hardening).
//
// The Node host has no OS keychain primitive, so it asks the renderer — which
// forwards to the Rust shell's DPAPI commands — to seal/unseal each secret
// value against the current Windows user. Sealed values carry a `dpapi:` marker
// so a value WITHOUT it is treated as legacy/fallback plaintext: secrets stored
// before this landed, or a build where DPAPI was unavailable.
//
// If the relay fails (renderer offline, non-Windows shell), we degrade to the
// documented plaintext fallback rather than lose the secret.

const MARKER = "dpapi:"

/** requestRenderer-shaped relay; only the two secret methods are used. */
export type SecretRelay = <T>(method: string, ...args: unknown[]) => Promise<T>

/** Plaintext → on-disk form. Sealed (`dpapi:<hex>`) when DPAPI is reachable,
 *  else the raw plaintext (fallback). */
export async function sealSecret(relay: SecretRelay, value: string): Promise<string> {
  try {
    return MARKER + (await relay<string>("secret.encrypt", value))
  } catch {
    return value
  }
}

/** On-disk form → plaintext. Unmarked values are returned as-is (legacy
 *  plaintext); a sealed value that fails to unseal yields undefined (the secret
 *  is genuinely unavailable — e.g. a different user can't decrypt it). NB this
 *  differs from VS Code's safeStorage, which THROWS on a decrypt failure; we
 *  return undefined so `secrets.get` degrades to "absent" rather than rejecting. */
export async function unsealSecret(relay: SecretRelay, stored: string): Promise<string | undefined> {
  if (!stored.startsWith(MARKER)) return stored
  try {
    return await relay<string>("secret.decrypt", stored.slice(MARKER.length))
  } catch {
    return undefined
  }
}
