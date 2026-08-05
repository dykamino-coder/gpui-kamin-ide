// Wire contract between the Rust/GPUI shell and the kamin-host Node process.
// JSON frames over the configured transport — one
// shared module so both sides compile against the same shape.
//
// Three frame kinds:
//   req — caller wants a method result (id matches the res)
//   res — reply to a req (ok=false carries a serialised error message)
//   evt — fire-and-forget broadcast (no reply, no ordering guarantee
//         beyond the underlying channel's FIFO)

export interface RpcRequest {
  kind: "req"
  id: number
  method: string
  params: unknown[]
}

export interface RpcResponse {
  kind: "res"
  id: number
  ok: boolean
  value?: unknown
  error?: string
}

export interface RpcEvent {
  kind: "evt"
  channel: string
  payload: unknown
}

export type RpcFrame = RpcRequest | RpcResponse | RpcEvent

/** Events the kamin-host emits toward the shell. */
/** Порог диагностики «медленный кадр» RPC/WS-пайплайна, мс. Общий для
 *  exthost-bridge/parent и ws-server: тюнить в ОДНОМ месте (ревью). */
export const SLOW_FRAME_MS = 150

export const EVT_BROADCAST = "kamin-host:broadcast"
export const EVT_READY = "kamin-host:ready"
export const EVT_FATAL = "kamin-host:fatal"

/** Methods the shell exposes to the kamin-host (renderer-facing UX). */
export const SHELL_SHOW_MESSAGE = "shell.showMessage"
export const SHELL_SHOW_INPUT_BOX = "shell.showInputBox"
export const SHELL_SHOW_QUICK_PICK = "shell.showQuickPick"
export const SHELL_SHOW_OPEN_DIALOG = "shell.showOpenDialog"
export const SHELL_SHOW_SAVE_DIALOG = "shell.showSaveDialog"
export const SHELL_OPEN_EXTERNAL = "shell.openExternal"
export const SHELL_READ_CLIPBOARD = "shell.readClipboard"

/** Methods the kamin-host exposes to the shell. */
export const HOST_SNAPSHOT = "host.snapshot"
export const HOST_EXECUTE_COMMAND = "host.executeCommand"
export const HOST_LIST_EXTENSIONS = "host.listExtensions"
export const HOST_DISPOSE = "host.dispose"

/** Host-internal method the shell calls after its folder-picker dialog. */
export const HOST_WORKSPACE_SET = "workspace.set"

/** Renderer-facing channels the shell forwards to kamin-host 1:1 — the
 *  RPC method name IS the channel name, so forwarding is a loop, not a
 *  hand-written map. invoke = request/response; send = fire-and-forget. */
export const FORWARDED_INVOKE_CHANNELS = [
  "kamin:workspace:get",
  "kamin:workspace:close",
  "kamin:fs:listDir",
  "kamin:fs:readText",
  "kamin:fs:writeText",
  "kamin:fs:delete",
  "kamin:fs:move",
  "kamin:index:findFile",
  "kamin:index:findInFiles",
  "kamin:shells:list",
  "kamin:pty:create",
  "kamin:pty:dispose",
] as const

export const FORWARDED_SEND_CHANNELS = [
  "kamin:pty:write",
  "kamin:pty:resize",
] as const

export function isRpcFrame(v: unknown): v is RpcFrame {
  if (typeof v !== "object" || v === null) return false
  const kind = (v as { kind?: unknown }).kind
  return kind === "req" || kind === "res" || kind === "evt"
}
