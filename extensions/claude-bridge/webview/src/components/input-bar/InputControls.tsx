import type { JSX } from 'preact'
import { EffortDropdown } from '../dropdown/EffortDropdown'
import { PermissionsDropdown } from '../dropdown/PermissionsDropdown'
import { WorkspaceApprovalsDropdown } from '../dropdown/WorkspaceApprovalsDropdown'
import { ModelDropdown } from '../dropdown/ModelDropdown'
import styles from './InputControls.module.css'

export function InputControls(): JSX.Element {
  return (
    <div class={`input-controls ${styles.controls}`} id="input-controls">
      <EffortDropdown />
      <PermissionsDropdown />
      <WorkspaceApprovalsDropdown />
      <span class="input-controls-spacer" />
      <ModelDropdown />
    </div>
  )
}
