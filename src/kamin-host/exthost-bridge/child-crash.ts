// JS-level crash containment for the ext-host CHILD (the Step-1 policy, now
// living where extensions actually run). A single post-boot uncaught throw /
// rejection is almost always ONE misbehaving extension's async code: log it +
// surface a toast and stay alive so the other extensions keep working. A boot
// error or a rapid crash-loop is genuine instability: exit so the PARENT's
// supervisor respawns a clean child (native segfaults bypass this entirely and
// are caught by the parent as an unexpected child exit).
const CRASH_WINDOW_MS = 10_000
const CRASH_LIMIT = 5

/** Install the handlers. Returns `markBooted`, called once the child has
 *  activated — before that, any error escalates to a clean exit + respawn. */
export function installChildCrashContainment(
  broadcast: (channel: string, payload: unknown) => void,
): () => void {
  let booted = false
  let crashTimes: number[] = []

  const onUncaught = (err: unknown, kind: string): void => {
    const message = err instanceof Error ? err.message : String(err)
    console.error(`exthost-child: ${kind}`, err)
    const now = Date.now()
    crashTimes = crashTimes.filter((t) => now - t < CRASH_WINDOW_MS)
    crashTimes.push(now)
    if (!booted || crashTimes.length > CRASH_LIMIT) {
      process.exit(1) // parent supervisor respawns a clean child
    }
    broadcast("kamin:notification:show", {
      severity: "error",
      title: "Extension crashed",
      message: `Contained — extensions stayed alive: ${message}`,
      timestamp: now,
    })
  }

  process.on("uncaughtException", (err) => { onUncaught(err, "uncaughtException") })
  process.on("unhandledRejection", (reason) => { onUncaught(reason, "unhandledRejection") })

  return () => { booted = true }
}
