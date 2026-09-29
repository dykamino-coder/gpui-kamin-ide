import { describe, expect, it } from "vitest"
import type { MessagePortLike } from "./port.js"
import type { RpcFrame } from "./protocol.js"
import { isPeerDisconnected, RpcEndpoint } from "./rpc.js"

/** In-memory loopback pair — frames posted on one side arrive on the
 *  other asynchronously (queueMicrotask), mirroring real port FIFO. */
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

describe("RpcEndpoint", () => {
  it("round-trips a call to a registered handler", async () => {
    const [a, b] = portPair()
    const caller = new RpcEndpoint(a)
    const callee = new RpcEndpoint(b)
    callee.handle("sum", (x, y) => (x as number) + (y as number))
    await expect(caller.call<number>("sum", 2, 3)).resolves.toBe(5)
  })

  it("propagates handler throws as rejected promises", async () => {
    const [a, b] = portPair()
    const caller = new RpcEndpoint(a)
    const callee = new RpcEndpoint(b)
    callee.handle("boom", () => { throw new Error("kaput") })
    await expect(caller.call("boom")).rejects.toThrow("kaput")
  })

  it("rejects calls to unknown methods", async () => {
    const [a, b] = portPair()
    const caller = new RpcEndpoint(a)
    void new RpcEndpoint(b)
    await expect(caller.call("nope")).rejects.toThrow("unknown method: nope")
  })

  it("supports async handlers", async () => {
    const [a, b] = portPair()
    const caller = new RpcEndpoint(a)
    const callee = new RpcEndpoint(b)
    callee.handle("later", async (v) => await Promise.resolve(v))
    await expect(caller.call<string>("later", "ok")).resolves.toBe("ok")
  })

  it("delivers fire-and-forget events to listeners", async () => {
    const [a, b] = portPair()
    const sender = new RpcEndpoint(a)
    const receiver = new RpcEndpoint(b)
    const got = new Promise<{ channel: string; payload: unknown }>((resolve) => {
      receiver.onEvent((channel, payload) => { resolve({ channel, payload }) })
    })
    sender.emit("hello", { x: 1 })
    await expect(got).resolves.toEqual({ channel: "hello", payload: { x: 1 } })
  })

  it("failAll rejects every in-flight call", async () => {
    const [a] = portPair() // peer never answers — b side has no endpoint
    const caller = new RpcEndpoint(a)
    const p1 = caller.call("hang")
    const p2 = caller.call("hang2")
    caller.failAll("peer died")
    await expect(p1).rejects.toThrow("peer died")
    await expect(p2).rejects.toThrow("peer died")
  })
})

describe("BR-19: разрыв соединения — отмена жизненного цикла, не падение", () => {
  it("failAll отклоняет типизированной отменой, а не безымянной ошибкой", async () => {
    const [a] = portPair()
    const caller = new RpcEndpoint(a)
    const pending = caller.call("showQuickPick")
    caller.failAll("shell client disconnected")
    await expect(pending).rejects.toSatisfy(isPeerDisconnected)
  })

  it("отмена переживает границу процесса: получатель видит код, а не текст", async () => {
    // Посредник повторяет реальную цепочку: ребёнок -> родитель -> оболочка.
    // Оболочка отваливается, родитель отдаёт ошибку обратно ребёнку.
    const [childSide, parentSide] = portPair()
    const [parentToShell, shellSide] = portPair()
    const child = new RpcEndpoint(childSide)
    const parent = new RpcEndpoint(parentSide)
    const toShell = new RpcEndpoint(parentToShell)
    const shell = new RpcEndpoint(shellSide)
    // Оболочка принимает вызов и НИКОГДА не отвечает: ответ придёт только
    // разрывом соединения, который мы и проверяем.
    const never = new Promise<never>(() => undefined)
    shell.handle("showInputBox", () => never)
    parent.handle("host:requestRenderer", (method) => toShell.call(method as string))

    const pending = child.call("host:requestRenderer", "showInputBox")
    await new Promise((r) => { setTimeout(r, 0) })
    toShell.failAll("shell client disconnected")

    await expect(pending).rejects.toSatisfy(isPeerDisconnected)
  })

  it("настоящая ошибка обработчика отменой НЕ считается", async () => {
    const [a, b] = portPair()
    const caller = new RpcEndpoint(a)
    const callee = new RpcEndpoint(b)
    callee.handle("boom", () => { throw new Error("shell client disconnected") })
    // Тот же человекочитаемый текст: классификация обязана идти по коду.
    await expect(caller.call("boom")).rejects.toSatisfy((e: unknown) => !isPeerDisconnected(e))
  })

  it("старое поколение клиента не отменяет вызов нового", async () => {
    const [oldSide] = portPair()
    const [newSide] = portPair()
    const oldGen = new RpcEndpoint(oldSide)
    const newGen = new RpcEndpoint(newSide)
    let settled = false
    const owned = newGen.call("showQuickPick")
    void owned.then(() => { settled = true }, () => { settled = true })
    const stale = oldGen.call("showQuickPick")
    void stale.catch(() => undefined)

    oldGen.failAll("shell client disconnected")
    await new Promise((r) => { setTimeout(r, 0) })

    expect(settled).toBe(false)
    await expect(stale).rejects.toSatisfy(isPeerDisconnected)
  })
})
