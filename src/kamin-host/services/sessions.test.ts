// Projects/Sessions service: CRUD, active-session fallback, persistence-backed
// state, the two notification surfaces (broadcast + in-process bus).
import { mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import * as sessions from "./sessions.js"

let dir: string
let prevAppData: string | undefined
const bc = vi.fn()

beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "kamin-sessions-"))
  // importBridgeSessions() reads $APPDATA (a GLOBAL path, not the injected
  // dataDir) to migrate a real Electron Bridge install's sessions. On a dev
  // machine that seeds dozens of real sessions and breaks the clean-slate
  // assertions (this suite passed only on a bare CI box with no such config).
  // Point APPDATA at the empty temp dir so the import finds nothing.
  prevAppData = process.env.APPDATA
  process.env.APPDATA = dir
  bc.mockClear()
  sessions.initSessions(dir, bc)
})
afterEach(() => {
  if (prevAppData === undefined) delete process.env.APPDATA
  else process.env.APPDATA = prevAppData
  vi.restoreAllMocks()
})

describe("sessions service", () => {
  it("newSessionInFolder creates a project + session, sets it active, broadcasts", () => {
    const s = sessions.newSessionInFolder("C:\\proj\\a")
    const snap = sessions.listSessions()
    expect(snap.projects).toHaveLength(1)
    expect(snap.sessions).toHaveLength(1)
    expect(snap.activeSessionId).toBe(s.id)
    expect(bc).toHaveBeenCalledWith("kamin:sessions:changed", expect.objectContaining({ activeSessionId: s.id }))
    expect(sessions.getActiveProjectFolder()).toBe("C:\\proj\\a")
  })

  it("newSessionInFolder dedupes the project but always adds a new session", () => {
    sessions.newSessionInFolder("C:\\proj\\a")
    const again = sessions.newSessionInFolder("C:\\proj\\a")
    const snap = sessions.listSessions()
    expect(snap.projects).toHaveLength(1) // same folder → one project
    expect(snap.sessions).toHaveLength(2) // but a fresh session each time
    expect(snap.activeSessionId).toBe(again.id)
  })

  it("newSession adds under the active project and activates it", () => {
    sessions.newSessionInFolder("C:\\proj\\a")
    const s2 = sessions.newSession()
    const snap = sessions.listSessions()
    expect(snap.sessions).toHaveLength(2)
    expect(snap.activeSessionId).toBe(s2.id)
    expect(snap.sessions.every((s) => s.projectId === snap.projects[0]?.id)).toBe(true)
  })

  it("newSession with no project creates a 'No folder' bucket", () => {
    const s = sessions.newSession()
    const snap = sessions.listSessions()
    expect(snap.projects).toHaveLength(1)
    expect(snap.projects[0]?.folderPath).toBeNull()
    expect(snap.activeSessionId).toBe(s.id)
    expect(sessions.getActiveProjectFolder()).toBeNull()
  })

  it("rename / setColor / setPinned mutate the session", () => {
    const s = sessions.newSession(undefined, "First")
    sessions.renameSession(s.id, "Renamed")
    sessions.setSessionColor(s.id, "#f38ba8")
    sessions.setSessionPinned(s.id, true)
    const got = sessions.listSessions().sessions.find((x) => x.id === s.id)
    expect(got).toMatchObject({ name: "Renamed", color: "#f38ba8", pinned: true })
  })

  it("deleting the active session falls back to another in the same project", () => {
    sessions.newSessionInFolder("C:\\proj\\a")
    const s2 = sessions.newSession()
    sessions.deleteSession(s2.id) // s2 was active
    const snap = sessions.listSessions()
    expect(snap.sessions).toHaveLength(1)
    expect(snap.activeSessionId).toBe(snap.sessions[0]?.id)
  })

  it("fires the in-process buses (onSessionsChange + onActiveSessionChange)", () => {
    const onChange = vi.fn()
    const onActive = vi.fn()
    sessions.onSessionsChange(onChange)
    sessions.onActiveSessionChange(onActive)
    const s = sessions.newSession()
    expect(onChange).toHaveBeenCalled()
    expect(onActive).toHaveBeenCalledWith(expect.objectContaining({ id: s.id }))
  })

  it("new sessions are open; deactivate frees them and reassigns active", () => {
    const s1 = sessions.newSessionInFolder("C:\\proj\\a")
    const s2 = sessions.newSession()
    expect(sessions.listSessions().sessions.every((s) => s.open)).toBe(true)
    sessions.deactivateSession(s2.id) // s2 was active
    const snap = sessions.listSessions()
    expect(snap.sessions.find((s) => s.id === s2.id)?.open).toBe(false)
    expect(snap.activeSessionId).toBe(s1.id) // fell back to the other open session
  })

  it("boot reconcile: a restored active session left open:false is re-opened (active ⟹ open)", () => {
    // A prior run / older build persisted `activeSessionId` pointing at a session
    // saved open:false — the "all inactive but a live chat on screen" state.
    const bootDir = mkdtempSync(join(tmpdir(), "kamin-sessions-boot-"))
    process.env.APPDATA = bootDir // keep the Electron-Bridge import empty
    writeFileSync(join(bootDir, "sessions.json"), JSON.stringify({
      projects: [{ id: "p1", name: "No folder", folderPath: null, createdAt: 1 }],
      sessions: [{ id: "s1", name: "Deep analysis", projectId: "p1", open: false, lastOpened: 1, createdAt: 1 }],
      activeSessionId: "s1",
    }))
    sessions.initSessions(bootDir, bc)
    const snap = sessions.listSessions()
    expect(snap.activeSessionId).toBe("s1")
    expect(snap.sessions.find((s) => s.id === "s1")?.open).toBe(true)
  })

  it("boot reconcile: a dangling activeSessionId (its session gone) is cleared", () => {
    const bootDir = mkdtempSync(join(tmpdir(), "kamin-sessions-boot2-"))
    process.env.APPDATA = bootDir
    writeFileSync(join(bootDir, "sessions.json"), JSON.stringify({
      projects: [], sessions: [], activeSessionId: "ghost",
    }))
    sessions.initSessions(bootDir, bc)
    expect(sessions.listSessions().activeSessionId).toBeNull()
  })

  it("setActiveSession reactivates an inactive session", () => {
    const s = sessions.newSession()
    sessions.deactivateSession(s.id)
    expect(sessions.listSessions().sessions[0]?.open).toBe(false)
    sessions.setActiveSession(s.id)
    expect(sessions.listSessions().sessions[0]?.open).toBe(true)
    expect(sessions.listSessions().activeSessionId).toBe(s.id)
  })

  it("moveSessionBefore reorders the stored array (null → end)", () => {
    const a = sessions.newSession(undefined, "A")
    sessions.newSession(undefined, "B")
    const c = sessions.newSession(undefined, "C")
    sessions.moveSessionBefore(c.id, a.id) // C before A
    expect(sessions.listSessions().sessions.map((s) => s.name)).toEqual(["C", "A", "B"])
    sessions.moveSessionBefore(c.id, null) // C to the end
    expect(sessions.listSessions().sessions.map((s) => s.name)).toEqual(["A", "B", "C"])
  })

  it("update shallow-merges metadata (Phase-3 control path)", () => {
    const s = sessions.newSession()
    sessions.updateSession(s.id, { metadata: { conv: "abc" } })
    sessions.updateSession(s.id, { metadata: { model: "opus" } })
    const got = sessions.listSessions().sessions.find((x) => x.id === s.id)
    expect(got?.metadata).toEqual({ conv: "abc", model: "opus" })
  })
})
