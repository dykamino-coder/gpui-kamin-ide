import { afterEach, describe, expect, it } from "vitest"
import { RpcPeerDisconnectedError } from "../rpc.js"
import { installChildCrashContainment } from "./child-crash.js"

/** Обработчики ставятся на `process`, поэтому каждый тест снимает свои: иначе
 *  они дожили бы до соседних файлов набора и считали чужие отказы. */
function install(): { toasts: unknown[]; markBooted: () => void; dispose: () => void } {
  const before = {
    uncaught: new Set(process.listeners("uncaughtException")),
    rejection: new Set(process.listeners("unhandledRejection")),
  }
  const toasts: unknown[] = []
  const markBooted = installChildCrashContainment((channel, payload) => {
    toasts.push({ channel, payload })
  })
  const dispose = (): void => {
    for (const fn of process.listeners("uncaughtException")) {
      if (!before.uncaught.has(fn)) process.off("uncaughtException", fn)
    }
    for (const fn of process.listeners("unhandledRejection")) {
      if (!before.rejection.has(fn)) process.off("unhandledRejection", fn)
    }
  }
  return { toasts, markBooted, dispose }
}

describe("BR-19: сдерживание падений отличает отмену от сбоя", () => {
  let active: { dispose: () => void } | null = null
  afterEach(() => { active?.dispose(); active = null })

  it("отмена по разрыву соединения не даёт ложного «Extension crashed»", () => {
    const h = install()
    active = h
    h.markBooted()

    process.emit("unhandledRejection", new RpcPeerDisconnectedError("shell client disconnected"), Promise.resolve())

    expect(h.toasts).toHaveLength(0)
  })

  it("настоящий отказ расширения по-прежнему доходит до сдерживания", () => {
    const h = install()
    active = h
    h.markBooted()

    process.emit("unhandledRejection", new Error("extension blew up"), Promise.resolve())

    expect(h.toasts).toHaveLength(1)
    expect(JSON.stringify(h.toasts[0])).toContain("extension blew up")
  })

  it("совпадение человекочитаемого текста отменой не считается", () => {
    // Тот же текст, что у настоящей отмены, но без кода: классификация обязана
    // идти по коду ошибки, иначе расширение сможет подделать отмену строкой.
    const h = install()
    active = h
    h.markBooted()

    process.emit("unhandledRejection", new Error("shell client disconnected"), Promise.resolve())

    expect(h.toasts).toHaveLength(1)
  })
})
