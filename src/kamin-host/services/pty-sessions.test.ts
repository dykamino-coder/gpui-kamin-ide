import * as pty from "@homebridge/node-pty-prebuilt-multiarch"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { PtySessions } from "./pty-sessions.js"

vi.mock("@homebridge/node-pty-prebuilt-multiarch", () => ({ spawn: vi.fn() }))
vi.mock("./shells.js", () => ({ discoverShells: () => Promise.resolve([]) }))

describe.skipIf(process.platform === "win32")("owned POSIX PTY disposal", () => {
  const kill = vi.fn<typeof process.kill>()
  beforeEach(() => {
    vi.useFakeTimers()
    kill.mockReturnValue(true)
    vi.spyOn(process, "kill").mockImplementation(kill)
  })
  afterEach(() => { vi.useRealTimers(); vi.restoreAllMocks(); vi.clearAllMocks() })

  function fixture() {
    const exited = vi.fn()
    const sessions = new PtySessions({ onData: vi.fn(), onExit: exited })
    async function spawn(pid: number) {
      let onExit!: (info: { exitCode: number; signal?: number }) => void
      const proc = {
        pid, kill: vi.fn(), write: vi.fn(), resize: vi.fn(), onData: vi.fn(),
        onExit: vi.fn((callback: typeof onExit) => { onExit = callback }),
      }
      vi.mocked(pty.spawn).mockReturnValueOnce(proc as unknown as pty.IPty)
      const id = await sessions.createSession({})
      return { id, proc, exit: () => { onExit({ exitCode: 0 }) } }
    }
    return { sessions, exited, spawn }
  }

  it("escalates once when the owned shell ignores graceful termination", async () => {
    const f = fixture(), a = await f.spawn(41001)
    f.sessions.disposeSession(a.id)
    f.sessions.disposeSession(a.id)
    f.sessions.disposeAllSessions()
    expect(kill).toHaveBeenCalledWith(-41001, "SIGTERM")
    expect(a.proc.kill.mock.calls).toEqual([[]])
    expect(vi.getTimerCount()).toBe(1)
    await vi.advanceTimersByTimeAsync(1000)
    expect(kill).toHaveBeenCalledWith(-41001, "SIGKILL")
    expect(a.proc.kill.mock.calls).toEqual([[], ["SIGKILL"]])
    expect(vi.getTimerCount()).toBe(0)
  })

  it("cancels escalation on exit and rejects writes after disposal", async () => {
    const f = fixture(), a = await f.spawn(41002)
    f.sessions.disposeSession(a.id)
    a.exit()
    f.sessions.writeToSession(a.id, "stale")
    f.sessions.resizeSession(a.id, 80, 24)
    await vi.advanceTimersByTimeAsync(1000)
    expect(a.proc.kill.mock.calls).toEqual([[]])
    expect(kill).toHaveBeenCalledTimes(1)
    expect(f.exited).toHaveBeenCalledExactlyOnceWith(a.id, 0, undefined)
    expect(a.proc.write).not.toHaveBeenCalled()
    expect(a.proc.resize).not.toHaveBeenCalled()
    expect(vi.getTimerCount()).toBe(0)
  })

  it("keeps disposal and natural exit isolated between sessions", async () => {
    const f = fixture(), a = await f.spawn(41003), b = await f.spawn(41004)
    f.sessions.disposeSession(a.id)
    b.exit()
    await vi.advanceTimersByTimeAsync(1000)
    expect(b.proc.kill).not.toHaveBeenCalled()
    expect(kill).not.toHaveBeenCalledWith(-41004, expect.anything())
    expect(a.proc.kill).toHaveBeenLastCalledWith("SIGKILL")
    a.exit()
    f.sessions.disposeAllSessions()
    expect(vi.getTimerCount()).toBe(0)
  })

  it("still kills the owned shell if its original process group has gone", async () => {
    const f = fixture(), a = await f.spawn(41005)
    kill.mockImplementation(() => { throw new Error("ESRCH") })
    f.sessions.disposeSession(a.id)
    await vi.advanceTimersByTimeAsync(1000)
    expect(a.proc.kill.mock.calls).toEqual([[], ["SIGKILL"]])
  })
})
