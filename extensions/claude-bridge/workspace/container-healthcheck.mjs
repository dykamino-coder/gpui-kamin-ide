// Reachability only: upstream auth/model readiness is a separate acceptance gate.
const url = new URL(process.env.WORKSPACE_ORIGIN)
url.protocol = 'http:'
url.hostname = '127.0.0.1'
url.port = process.env.WORKSPACE_PORT || '3460'
// This endpoint reports process readiness without initiating upstream login.
url.pathname = '/health'
try {
  const response = await fetch(url, {
    headers: { Host: new URL(process.env.WORKSPACE_ORIGIN).host },
    redirect: 'error', signal: AbortSignal.timeout(4000),
  })
  if (!response.ok) process.exitCode = 1
} catch { process.exitCode = 1 }
