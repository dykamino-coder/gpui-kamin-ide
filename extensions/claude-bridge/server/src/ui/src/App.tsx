import { useEffect } from 'preact/hooks'
import { lazy, Suspense } from 'preact/compat'
import { AppShell } from './components/layout/AppShell'
import { SingleDashboard } from './SingleDashboard'
import { CompactConfig } from './components/config/CompactConfig'
import { ApiTokens } from './components/tokens/ApiTokens'
import { RequestListModal } from './components/requests/RequestListModal'
import { SectionHeader, Modal } from './components/shared'
import { Login } from './components/auth/Login'
import { wsClient } from './services/ws-client'
import { showRequestList, requestListFilter, closeRequestList } from './signals/ui'
import { isAuthenticated, authChecked, checkSession } from './signals/auth'
import { currentRoute, terminalEverOpened } from './router'
import styles from './components/layout/AppShell.module.css'

const LazyTerminal = lazy(() => import('./components/terminal/Terminal').then(m => ({ default: m.Terminal })))

export function App() {
  const route = currentRoute.value

  // Check auth session on mount
  useEffect(() => { checkSession() }, [])

  useEffect(() => {
    if (!isAuthenticated.value) return
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const wsUrl = `${protocol}//${window.location.host}/ws/dashboard`
    wsClient.connect(wsUrl)
    return () => wsClient.disconnect()
  }, [isAuthenticated.value])

  // Show nothing while checking auth
  if (!authChecked.value) {
    return <div style={{ background: 'var(--bg-primary)', width: '100vw', height: '100vh' }} />
  }

  // Show login screen if not authenticated
  if (!isAuthenticated.value) {
    return <Login />
  }

  // Mark terminal as opened when navigating to console
  if (route === 'console' && !terminalEverOpened.value) {
    terminalEverOpened.value = true
  }

  return (
    <>
    <AppShell>
      {/* Dashboard View */}
      {route === 'dashboard' && (
        <div class={styles.content}>
          <SingleDashboard />
        </div>
      )}

      {/* Console / Terminal — always mounted once opened, hidden via CSS */}
      {terminalEverOpened.value && (
        <div class={route === 'console' ? styles.terminalWrap : styles.hidden}>
          <Suspense fallback={<div style={{ color: 'var(--text-muted)', padding: '20px' }}>Loading terminal...</div>}>
            <LazyTerminal />
          </Suspense>
        </div>
      )}

      {/* Configuration View */}
      {route === 'config' && (
        <div class={styles.content}>
          <CompactConfig />
        </div>
      )}

      {/* API Tokens View */}
      {route === 'tokens' && (
        <div class={styles.content}>
          <ApiTokens />
        </div>
      )}
    </AppShell>

    {/* Global Request List Modal — accessible from any route */}
    <Modal
      isOpen={showRequestList.value}
      onClose={closeRequestList}
      title={`Requests: ${requestListFilter.value.label}`}
      size="xl"
    >
      <RequestListModal filter={requestListFilter.value} />
    </Modal>
    </>
  )
}
