import type { JSX } from 'preact'
import { NewSessionButton } from './NewSessionButton'
import { CustomizeButton } from './CustomizeButton'
import { StatsButton } from './StatsButton'

export function SidebarTopActions(): JSX.Element {
  return (
    <div class="sidebar-top-actions">
      <NewSessionButton />
      <CustomizeButton />
      <StatsButton />
    </div>
  )
}
