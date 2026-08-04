import { currentRoute, navigate } from '../../router'
import type { Route } from '../../router'
import { terminalSessions } from '../../signals/server'
import { isAuthenticated, logout } from '../../signals/auth'
import styles from './Sidebar.module.css'

interface NavEntry {
  route: Route
  icon: string
  label: string
}

const navItems: NavEntry[] = [
  { route: 'dashboard', icon: 'fa-solid fa-gauge', label: 'Dashboard' },
  { route: 'console', icon: 'fa-solid fa-terminal', label: 'Console' },
  { route: 'config', icon: 'fa-solid fa-gear', label: 'Configuration' },
  { route: 'tokens', icon: 'fa-solid fa-key', label: 'API Tokens' },
]

function BridgeLogo() {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2048 1380" width="42" height="42" style={{ display: 'block' }}>
      <path fill="currentColor" d="M353 13.3c35.4-1.5 79 -.3 115-.3l214.5 0 653.5 0 280.1 0 67.9 0c12.5 0 37 -.9 47.8 1.9 4.2 4.3 2.5 76.3 2.5 89.2l-.1 164.2c0 13 -.9 58.2 1.6 66.8 4.8 3.2 73.1 1.9 83.2 1.9l152.4-.1c9.1 0 64.1-2.2 64.3 4l1.3 324.5c0 3.7-1.3 12.1-6 12.3-22 .8-44.2.5-66.3.5l-172.1 0c-9 0-49.4-.5-55.2.9-2 1.8-2.9 2.5-2.9 5.8-.3 20.1-.2 40.7-.2 60.8l0 121 0 380.4 0 75.5c0 12 .6 32.8-.8 43.8-7.9 8-130.5 1.6-148.5 3.8-5.4.7-40.6.3-45.8-1.5-6.6-10.1 .8-312.2-3.1-357.3-10.1-1.9-50.5-1.1-62.7-1.1-30.8-.1-63.1-.2-93.9.3-1.1 0-7.4.8-7.5 1.1-2 3.6-1.9 14.7-1.9 18.5l0 226.2c0 34.4 0 69 0 103.5-.3 1.8 0 6.9-1.8 8-7.3 4.6-22.4 3-30.6 2.8l-90.7.1c-10.7 0-38.4.6-47.3-1.1-3.5-4.3-2.6-28.6-2.7-35.5-1.5-107.2 2.1-215.6-.3-322.8-10.4-1.9-46-.2-57.7-.2l-113.4 0-103.9 0c-17.1 0-35.6-.6-52.6 1l-5.3 10.6c-.3 29.6-.2 59.5-.2 89.1l0 166.4c0 12.7 2 84.5-2.6 90.8-17.6 4.1-72.2 1.2-93 2l-69 -.4c-10.6-.3-9.6-8.5-9.5-17.1l.3-217.4 0-71.5c0-9.1 1.3-47.2-3.2-52.5-10.1-2.1-153.6-2.2-158.7-.2-3.3 1.3-3.8 5.7-4.1 8.8-1 9.7-.2 20.1-.2 29.9l.1 55.5.0 186 0 13.7c-3.3 1.4-6.6 1.5-10.1 1.6-15.4.6-147.2 1.3-154.6-1.1-5.4-1.8-3.9-13-3.9-17.8l.1-443.3 0-146.7c0-11.8 1.1-75.4-1.5-81-8.3-3.3-51.3-2.2-62.9-2.2l-189 0-55.8 0c-7.9 0-15.8.2-23.7 0-5.7-.1-5.5-8.5-5.5-12.4-.3-92.2-.1-184.5-.1-276.7.4-8.5-.8-41.9.8-47.6 1.7-6.3 85.4-4 92.6-4l153.7.1c23.4 0 64.8 1.6 88.7-2 3.8-.6 2.8-19.7 2.8-24.6l0-221.5c0-12.5-1.4-69.1 1.1-76z"/>
      <path fill="#0a0c0f" d="M1379 168.5c52.6 0 107 -.8 159.3.0.2 6.5.6 34.4.1 39.2-1.2 13.7 1.1 122.8-1.9 127.2l-4.2.5c-53.7 0-108.1 1.2-161.6.2-1.4-32.6.2-69.7-.5-102.7-.2-12.1-.8-52.2.5-62.6 2.8-2.1 3.5-1.5 7.4-1.7z"/>
      <path fill="#0a0c0f" d="M535.4 168.8c2.6-.3 26.6-.3 28.5 0 19.1 2.5 119.7-4.1 132.7 3.3 3 9.1 1.1 54.2 1 67l-.3 56.1c0 9.8 1.6 32.5-3.6 39.6-38.8 1.5-80.1-.9-119 0-10.9.3-33.9 1-42.9-3.4-1.7-2.2-1.8-6-1.7-8.9.0-30 0-59.4.7-89.4.4-18.5-1.4-39.2.5-57.7.3-2.8 2.2-4.7 4.2-6.7z"/>
    </svg>
  )
}

export function Sidebar() {
  const active = currentRoute.value

  return (
    <aside class={styles.sidebar}>
      <div class={styles.logo} title="Open Claude Max Bridge">
        <BridgeLogo />
      </div>

      <nav class={styles.nav}>
        {navItems.map((item) => (
          <button
            key={item.route}
            class={active === item.route ? styles.navItemActive : styles.navItem}
            onClick={() => navigate(item.route)}
            title={item.label}
          >
            <i class={item.icon} />
            {item.route === 'console' && terminalSessions.value > 0 && (
              <span class={styles.badge}>{terminalSessions.value}</span>
            )}
            <span class={styles.tooltip}>{item.label}</span>
          </button>
        ))}
      </nav>

      {isAuthenticated.value && (
        <button
          class={styles.navItem}
          onClick={logout}
          title="Sign Out"
          style={{ marginTop: 'auto', flexShrink: 0 }}
        >
          <i class="fa-solid fa-right-from-bracket" style={{ transform: 'scaleX(-1)' }} />
          <span class={styles.tooltip}>Sign Out</span>
        </button>
      )}
    </aside>
  )
}
