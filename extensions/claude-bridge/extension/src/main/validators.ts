// Input validation helpers for child_process + filesystem ops.
// Reject anything that could break out of an argv slot into a shell context.
// We're in the VSIX bridge host (the user's own machine), so this is defense-in-depth —
// the attacker model here is a malicious marketplace URL / folder name that
// slipped past UI validation, not a general RCE vector.

import path from 'path'

const VALID_NAME = /^[A-Za-z0-9._-]{1,64}$/
// Allow https / http / git@ / ssh:// / git:// schemes. Corporate GitLab
// instances often expose HTTP-only endpoints behind a VPN, so rejecting
// http:// breaks legit internal marketplaces. Keep argv-safe character
// class (no shell metacharacters).
const VALID_GIT_URL = /^(https?:\/\/|git@|ssh:\/\/|git:\/\/)[A-Za-z0-9._\-\/:@+%~]{4,512}(\.git)?$/

export function assertValidName(s: string, field: string): void {
  if (typeof s !== 'string' || !VALID_NAME.test(s)) throw new Error(`Invalid ${field}: ${s}`)
}

export function assertValidGitUrl(u: string): void {
  if (typeof u !== 'string' || !VALID_GIT_URL.test(u)) throw new Error(`Invalid git URL: ${u}`)
}

export function assertAbsolutePath(p: string, field: string): void {
  if (typeof p !== 'string' || !path.isAbsolute(p)) throw new Error(`Invalid ${field}: ${p}`)
}
