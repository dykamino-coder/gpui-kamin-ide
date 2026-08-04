import type { JSX } from 'preact'
import { SidebarTopActions } from './SidebarTopActions'
import { ProjectsHeader } from './ProjectsHeader'
import { SidebarTree } from './SidebarTree'

export function SessionsMode(): JSX.Element {
  return (
    <>
      <SidebarTopActions />
      <ProjectsHeader />
      <SidebarTree />
    </>
  )
}
