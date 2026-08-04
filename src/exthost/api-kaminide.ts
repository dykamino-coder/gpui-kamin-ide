// The `kaminide` virtual module (Phase 3) — what `require('kaminide')` resolves
// to inside an extension. ADDITIVE to `vscode`: an extension that never requires
// it is unaffected, and it is NOT exposed as `vscode.kaminide` (which would
// violate the faithful `@types/vscode` mirror). A single shared instance backs
// every caller because session events are global, not per-extension.
import type { SessionsApi } from "./api/sessions.js"

/** `kaminide.version` — the API contract version, bumped when the surface in
 *  kaminide.d.ts changes incompatibly (extensions feature-detect on it). */
const KAMINIDE_API_VERSION = "2.0.0"

export interface KaminideApi {
  readonly version: string
  readonly sessions: SessionsApi
}

export function createKaminideApi(sessions: SessionsApi): KaminideApi {
  return { version: KAMINIDE_API_VERSION, sessions }
}
