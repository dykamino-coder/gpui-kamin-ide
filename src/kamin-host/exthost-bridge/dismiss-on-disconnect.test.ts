import { describe, expect, it } from "vitest"
import type { MessagePortLike } from "../port.js"
import type { RpcFrame } from "../protocol.js"
import { RpcEndpoint } from "../rpc.js"
import { dismissOnPeerDisconnect } from "./dismiss-on-disconnect.js"

/** Подмена транспорта, а не помощника: кадры ходят через настоящий
 *  `RpcEndpoint`, поэтому проверяется тот же путь, которым идут диалоги в
 *  живом ребёнке — включая перенос кода ошибки через границу процесса. */
function portPair(): [MessagePortLike, MessagePortLike] {
  const aListeners: ((f: RpcFrame) => void)[] = []
  const bListeners: ((f: RpcFrame) => void)[] = []
  const a: MessagePortLike = {
    post: (f) => { queueMicrotask(() => { for (const fn of bListeners) fn(f) }) },
    onFrame: (fn) => { aListeners.push(fn) },
  }
  const b: MessagePortLike = {
    post: (f) => { queueMicrotask(() => { for (const fn of aListeners) fn(f) }) },
    onFrame: (fn) => { bListeners.push(fn) },
  }
  return [a, b]
}

/** Цепочка живого ребёнка: `child -> parent -> shell`. Родитель проксирует
 *  запрос оболочке ровно так же, как `parent.ts` на `HOST_REQUEST_RENDERER`. */
function childParentShell(): {
  requestRenderer: <T>(method: string) => Promise<T>
  dropShell: () => void
  failChildSide: () => void
} {
  const [childSide, parentSide] = portPair()
  const [parentToShell, shellSide] = portPair()
  const child = new RpcEndpoint(childSide)
  const parent = new RpcEndpoint(parentSide)
  const toShell = new RpcEndpoint(parentToShell)
  const shell = new RpcEndpoint(shellSide)
  const never = new Promise<never>(() => undefined)
  shell.handle("showInputBox", () => never)
  shell.handle("showQuickPick", () => never)
  shell.handle("boom", () => { throw new Error("extension blew up") })
  parent.handle("host:requestRenderer", (method) => toShell.call(method as string))
  return {
    requestRenderer: <T,>(method: string) => child.call<T>("host:requestRenderer", method),
    dropShell: () => { toShell.failAll("shell client disconnected") },
    failChildSide: () => { child.failAll("shell client disconnected") },
  }
}

describe("BR-19: закрываемый диалог при разрыве соединения", () => {
  it("showInputBox завершается как «закрыт пользователем», а не отказом", async () => {
    const link = childParentShell()
    const pending = dismissOnPeerDisconnect(link.requestRenderer<string | undefined>("showInputBox"), undefined)
    await new Promise((r) => { setTimeout(r, 0) })

    link.dropShell()

    await expect(pending).resolves.toBeUndefined()
  })

  it("showQuickPick отдаёт значение отмены своего контракта", async () => {
    // У выборки «отменено» выражается как `null`, а не `undefined`: значение
    // отмены задаёт вызывающий, помощник его не выдумывает.
    const link = childParentShell()
    const pending = dismissOnPeerDisconnect(link.requestRenderer<number[] | null>("showQuickPick"), null)
    await new Promise((r) => { setTimeout(r, 0) })

    link.dropShell()

    await expect(pending).resolves.toBeNull()
  })

  it("настоящая ошибка расширения проходит насквозь и отменой не подменяется", async () => {
    const link = childParentShell()

    await expect(dismissOnPeerDisconnect(link.requestRenderer<string>("boom"), "подменено"))
      .rejects.toThrow("extension blew up")
  })

  it("успешный ответ значением отмены не подменяется", async () => {
    const [a, b] = portPair()
    const caller = new RpcEndpoint(a)
    const callee = new RpcEndpoint(b)
    callee.handle("showInputBox", () => "введено пользователем")

    await expect(dismissOnPeerDisconnect(caller.call<string>("showInputBox"), undefined as unknown as string))
      .resolves.toBe("введено пользователем")
  })

  it("обрыв на стороне самого ребёнка тоже считается закрытием", async () => {
    // Рвётся не связь родителя с оболочкой, а канал ребёнок→родитель:
    // диалога всё равно нет, и контракт обязан завершиться одинаково.
    const link = childParentShell()
    const pending = dismissOnPeerDisconnect(link.requestRenderer<string | undefined>("showInputBox"), undefined)
    await new Promise((r) => { setTimeout(r, 0) })

    link.failChildSide()

    await expect(pending).resolves.toBeUndefined()
  })
})
