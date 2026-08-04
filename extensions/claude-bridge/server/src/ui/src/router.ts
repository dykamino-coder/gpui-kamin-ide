import { signal } from '@preact/signals'

export type Route = 'dashboard' | 'console' | 'config' | 'tokens'

const VALID_ROUTES: Route[] = ['dashboard', 'console', 'config', 'tokens']
const BASE = '/ui'

function parsePath(): Route {
  const path = location.pathname.replace(BASE, '').replace(/^\//, '') || 'dashboard'
  if (VALID_ROUTES.includes(path as Route)) return path as Route
  return 'dashboard'
}

export const currentRoute = signal<Route>(parsePath())

window.addEventListener('popstate', () => {
  currentRoute.value = parsePath()
})

export function navigate(route: Route) {
  const url = route === 'dashboard' ? BASE : `${BASE}/${route}`
  history.pushState(null, '', url)
  currentRoute.value = route
}

// Terminal must stay mounted once opened to preserve PTY session
export const terminalEverOpened = signal(false)

export function navigateToConsole() {
  terminalEverOpened.value = true
  navigate('console')
}
