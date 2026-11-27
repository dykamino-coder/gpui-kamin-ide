// Delay external processes during bare host activation. Once a chat has a
// socket, give ready connectors a short head start on the engine's first list.
// A slow handshake continues in the background for the ordinary live refresh.
export function mcpStartup(initialize: () => Promise<void>) {
  let started = false
  let disposed = false
  let release!: () => void
  const admission = new Promise<void>(resolve => { release = resolve })
  let deadline: ReturnType<typeof setTimeout> | undefined
  const fallback = setTimeout(start, 30_000)

  function finish() {
    clearTimeout(deadline)
    release()
  }
  function start() {
    if (started || disposed) return
    started = true
    clearTimeout(fallback)
    deadline = setTimeout(finish, 1500)
    void initialize().then(finish, finish)
  }
  return {
    beforeAdmission() {
      start()
      return admission
    },
    dispose() {
      disposed = true
      clearTimeout(fallback)
      finish()
    },
  }
}
