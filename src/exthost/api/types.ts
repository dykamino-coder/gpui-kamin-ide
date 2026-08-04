import type { URI } from "vscode-uri"

/** Public-facing `vscode.Extension<T>` shape. Mirrors d.ts §8338.
 *
 *  Internal mutators (`__setActive`, `__setExports`) live on the
 *  concrete object the loader hands out — they're not in this type so
 *  consumers stay locked to the documented contract. The loader casts
 *  through `unknown` when it needs to flip them. */
export interface ExtensionFacade {
  readonly id: string
  readonly extensionUri: URI
  readonly extensionPath: string
  readonly isActive: boolean
  readonly packageJSON: Record<string, unknown>
  readonly extensionKind: number
  readonly exports: unknown
  activate(): Promise<unknown>
}
