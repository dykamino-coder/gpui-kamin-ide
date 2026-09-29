// INC-2026-0055: отказ вещания терял пачку постов без исхода.
//
// `sendOneFrame` вынимает пачку из очереди ДО вещания и завершает её только
// после. Если вещание бросит — а оно бросает: отказ записи в канал IPC ребёнка
// приходит именно отсюда (`write UNKNOWN`) — пачка исчезала бесследно, и её
// обещания не завершались никогда. Контракт `postMessage` знает такой исход:
// он разрешается и при доставке, и когда сообщение потеряно.
import { describe, expect, it } from "vitest"
import { Webviews } from "./webview.js"

/** Обещание, которое обязано завершиться: тест не имеет права висеть, если
 *  исправление снято, поэтому ожидание ограничено. */
function settledWithin<T>(p: Promise<T>, ms = 200): Promise<T | "не завершилось"> {
  return Promise.race([
    p,
    new Promise<"не завершилось">((r) => {
      setTimeout(() => { r("не завершилось") }, ms)
    }),
  ])
}

type Broadcast = (channel: string, payload: unknown) => void

/** Вещание, которое бросает ровно так, как отказавшая запись в канал IPC. */
function throwingBroadcast(): Broadcast {
  return (channel: string) => {
    if (channel === "kamin:webview:post") {
      const err = new Error("write UNKNOWN") as Error & { code: string }
      err.code = "UNKNOWN"
      throw err
    }
  }
}

describe("INC-2026-0055: пост получает исход даже при отказе вещания", () => {
  it("отказ завершает пост как недоставленный, а не оставляет висеть", async () => {
    const bc = throwingBroadcast()
    const panel = new Webviews(bc).createPanel("v", "t", 1, {})
    const posted = panel.webview.postMessage({ hello: "world" })

    await expect(settledWithin(posted)).resolves.toBe(false)
  })

  it("вся пачка получает исход, а не только первый пост", async () => {
    const bc = throwingBroadcast()
    const panel = new Webviews(bc).createPanel("v", "t", 1, {})

    const all = [1, 2, 3].map((n) => panel.webview.postMessage({ n }))

    await expect(Promise.all(all.map((p) => settledWithin(p)))).resolves.toEqual([false, false, false])
  })

  it("успешное вещание по-прежнему завершает пост доставленным", async () => {
    // Сторож: исход `false` не должен появиться на здоровом пути.
    const bc: Broadcast = () => undefined
    const panel = new Webviews(bc).createPanel("v", "t", 1, {})

    await expect(settledWithin(panel.webview.postMessage({ ok: true }))).resolves.toBe(true)
  })

  it("синхронный сброс перед dispose тоже не оставляет очередь без исхода", async () => {
    // `flushNow` гонит очередь в цикле: бросок на первом же кадре оставлял
    // остальные посты в очереди навсегда.
    const bc = throwingBroadcast()
    const panel = new Webviews(bc).createPanel("v", "t", 1, {})
    const posts = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10].map((n) => panel.webview.postMessage({ n }))
    try {
      panel.dispose()
    } catch {
      /* отказ вещания виден вызывающему — это и требуется */
    }

    await expect(Promise.all(posts.map((p) => settledWithin(p)))).resolves.toEqual(
      posts.map(() => false),
    )
  })
})
