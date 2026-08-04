import { describe, expect, it } from "vitest"
import type { MessagePortLike } from "./port.js"
import type { RpcFrame } from "./protocol.js"
import { RpcEndpoint } from "./rpc.js"

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
