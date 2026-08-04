import { ComponentChildren } from 'preact'
import { Sidebar } from './Sidebar'
import { RightSidebar } from './RightSidebar'
import { Header } from './Header'
import { currentRoute } from '../../router'
import styles from './AppShell.module.css'

interface AppShellProps {
  children: ComponentChildren
}

export function AppShell({ children }: AppShellProps) {
  const isConsole = currentRoute.value === 'console'

  return (
    <div class={styles.shell}>
      <Sidebar />
      <div class={styles.mainColumn}>
        <Header />
        <main class={isConsole ? styles.mainFullBleed : styles.main}>
          {children}
        </main>
      </div>
      <RightSidebar />
    </div>
  )
}
