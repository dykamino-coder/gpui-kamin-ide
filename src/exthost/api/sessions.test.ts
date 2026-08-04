// SessionsApi (Phase 3) — the registry that diffs host snapshots into typed
// `kaminide.sessions` events. Drives it through a controllable fake SessionsHost.
import { describe, expect, it } from "vitest"
import type { KaminSession, SessionsSnapshot } from "../../api/types.js"
import type { SessionsHost } from "../host-services.js"
import { SessionsApi, type SessionsChangeEvent } from "./sessions.js"

function sess(id: string, over: Partial<KaminSession> = {}): KaminSession {
  return { id, name: id, projectId: "p", lastOpened: 0, createdAt: 0, ...over }
}
const snap = (sessions: KaminSession[], activeSessionId: string | null = null): SessionsSnapshot =>
  ({ projects: [], sessions, activeSessionId })

function makeFakeHost() {
  let cur: SessionsSnapshot = snap([])
  const changeListeners = new Set<(s: SessionsSnapshot) => void>()
  const activeListeners = new Set<(s: KaminSession | null) => void>()
  const host: SessionsHost = {
    list: () => cur,
    getActive: () => cur.sessions.find((s) => s.id === cur.activeSessionId) ?? null,
    onChange: (fn) => { changeListeners.add(fn); return { dispose: () => { changeListeners.delete(fn) } } },
    onActiveChange: (fn) => { activeListeners.add(fn); return { dispose: () => { activeListeners.delete(fn) } } },
    createSession: () => { throw new Error("unused") },
    setActiveSession: () => { /* unused */ },
    updateSession: () => Promise.resolve(null),
    deleteSession: () => { /* unused */ },
  }
  return {
    host,
    // Real host fires changeListeners BEFORE activeListeners; mirror that.
    push(next: SessionsSnapshot) { cur = next; for (const fn of changeListeners) fn(next) },
    pushActive(s: KaminSession | null) { for (const fn of activeListeners) fn(s) },
  }
}

describe("SessionsApi", () => {
  it("serves snapshot getters from the captured snapshot", () => {
    const fake = makeFakeHost()
    fake.push(snap([sess("a"), sess("b")], "b"))
    const api = new SessionsApi(fake.host)
    expect(api.all.map((s) => s.id)).toEqual(["a", "b"])
    expect(api.active?.id).toBe("b")
    expect(api.getSession("a")?.id).toBe("a")
    expect(api.getSession("zzz")).toBeUndefined()
  })

  it("diffs added / removed / changed and fires onDidChangeSessions once", () => {
    const fake = makeFakeHost()
    fake.push(snap([sess("a"), sess("b", { name: "B" })]))
    const api = new SessionsApi(fake.host)
    const events: SessionsChangeEvent[] = []
    api.onDidChangeSessions((e) => events.push(e))
    fake.push(snap([sess("b", { name: "B2" }), sess("c")])) // a removed, c added, b changed
    expect(events).toHaveLength(1)
    expect(events[0]?.added.map((s) => s.id)).toEqual(["c"])
    expect(events[0]?.removed.map((s) => s.id)).toEqual(["a"])
    expect(events[0]?.changed.map((s) => s.id)).toEqual(["b"])
  })

  it("does NOT fire changed when only metadata key order differs", () => {
    const fake = makeFakeHost()
    fake.push(snap([sess("a", { metadata: { x: 1, y: 2 } })]))
    const api = new SessionsApi(fake.host)
    let fired = 0
    api.onDidChangeSessions(() => { fired += 1 })
    fake.push(snap([sess("a", { metadata: { y: 2, x: 1 } })])) // same content, reordered
    expect(fired).toBe(0)
  })

  it("fires onDidChangeActiveSession with the new session", () => {
    const fake = makeFakeHost()
    fake.push(snap([sess("a")], "a"))
    const api = new SessionsApi(fake.host)
    const seen: (string | undefined)[] = []
    api.onDidChangeActiveSession((s) => seen.push(s?.id))
    fake.pushActive(sess("a"))
    fake.pushActive(null)
    expect(seen).toEqual(["a", undefined])
  })

  it("dispose() detaches the host subscriptions", () => {
    const fake = makeFakeHost()
    const api = new SessionsApi(fake.host)
    let fired = 0
    api.onDidChangeSessions(() => { fired += 1 })
    api.dispose()
    fake.push(snap([sess("a")]))
    expect(fired).toBe(0)
  })
})
