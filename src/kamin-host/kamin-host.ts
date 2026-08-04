// kamin-host entry — a thin role dispatcher. One bundle serves two processes:
//   • parent (default): owns the user-facing services (PTY, fs, index, sessions)
//     and the renderer WS link; forks the child for crash isolation.
//   • child (`--role=exthost`): runs ONLY the extension host, so a native crash
//     in an addon takes down just this process and the parent respawns it.
//
// The heavy service/exthost graphs are pulled via dynamic import INSIDE the
// chosen branch so neither role drags in the other's boot work. `openChildPort`
// auto-detects the transport: stdio (parent ↔ Rust shell) vs fork-IPC
// (child ↔ parent), so the same RpcEndpoint wiring serves both peers.
import { EXTHOST_ROLE_ARG, EXTHOST_ROLE_VALUE } from "./exthost-bridge/protocol.js"
import { openChildPort } from "./port.js"
import { EVT_FATAL } from "./protocol.js"
import { RpcEndpoint } from "./rpc.js"

// Монитор блокировок цикла РОДИТЕЛЯ: у ребёнка блокировок ноль, а RPC отвечают
// через 10-25 с — значит держит этот процесс. Тик 50 мс, лог при задержке >400.
{
  // Этот файл — ОДИН И ТОТ ЖЕ для родителя и для форкнутого exthost-ребёнка
  // (fork(selfScript)), поэтому монитор помечает роль: без метки 18 блоков
  // выглядели родительскими, хотя могли приходить из ребёнка.
  const role = process.argv.some((a) => a.includes(EXTHOST_ROLE_VALUE)) ? "child" : "parent"
  const t0 = Date.now()
  const TICK_MS = 50
  const LAG_MS = 400
  let last = Date.now()
  const t = setInterval(() => {
    const now = Date.now()
    const lag = now - last - TICK_MS
    if (lag > LAG_MS) console.error(`[loop блок ${role}] ${lag}ms на ${now - t0}ms от старта`)
    last = now
  }, TICK_MS)
  t.unref()
}

function argValue(name: string): string | null {
  const prefix = `--${name}=`
  const hit = process.argv.find((a) => a.startsWith(prefix))
  return hit ? hit.slice(prefix.length) : null
}

const endpoint = new RpcEndpoint(openChildPort())
const isChild = argValue(EXTHOST_ROLE_ARG) === EXTHOST_ROLE_VALUE

const boot = isChild
  ? import("./exthost-bridge/child.js").then((m) => m.runExtHostChild(endpoint))
  : import("./host-main.js").then((m) => m.runHost(endpoint))

boot.catch((err: unknown) => {
  const message = err instanceof Error ? err.message : String(err)
  console.error("kamin-host: boot failed", err)
  // Meaningful for the parent (the Rust shell listens for EVT_FATAL); harmless
  // for the child (the parent ignores it and respawns on the process exit).
  endpoint.emit(EVT_FATAL, { message })
  process.exit(1)
})
