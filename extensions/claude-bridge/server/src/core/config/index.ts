// ============================================================================
// Configuration — priority: CLI args > env vars > opencode.json > defaults
// ============================================================================

import { readFileSync } from "node:fs"
import type { Config } from "../types"
import { args } from "../utils/args"
import { readBridgeSettings } from "./opencode"

const DEFAULTS = { port: 3456, host: "127.0.0.1" }

// Read what's already in opencode.json (may have user-set port/host)
const opencode = readBridgeSettings()

export const config: Config = {
  port: args.port
    ?? (process.env.CLAUDE_PROXY_PORT && Number.isFinite(parseInt(process.env.CLAUDE_PROXY_PORT, 10)) ? parseInt(process.env.CLAUDE_PROXY_PORT, 10) : undefined)
    ?? opencode.port
    ?? DEFAULTS.port,

  host: args.host
    ?? process.env.CLAUDE_PROXY_HOST
    ?? opencode.host
    ?? DEFAULTS.host,

  debug: args.debug
    || process.env.DEBUG === "true" || process.env.DEBUG === "1"
    || process.env.CLAUDE_PROXY_DEBUG === "1",

  verbose: args.verbose
    || process.env.VERBOSE === "true" || process.env.VERBOSE === "1",
}

const packageManifest = JSON.parse(
  readFileSync(new URL("../../../package.json", import.meta.url), "utf-8"),
) as { version?: unknown }

if (
  typeof packageManifest.version !== "string" ||
  packageManifest.version.length === 0
) {
  throw new Error("Bridge package.json must contain a non-empty version")
}

// package.json is the release source of truth; keeping a second literal here
// allowed the dashboard and health endpoints to report an older server build.
export const VERSION = packageManifest.version
export const SERVICE_NAME = "open-claude-bridge"

// The model a fresh session launches with when the client picks nothing.
// --resume'd sessions keep their own historical model; this only seeds
// brand-new ones. Opus 5 ships a 1M context NATIVELY (like Fable 5) — the
// `[1m]` variant ids are gone; 4.8 is retired entirely (as 4.7 was before).
export const DEFAULT_SESSION_MODEL = "claude-opus-5"
