import { signal } from '@preact/signals'
import { inv, sub } from '../../lib/bridge-transport'
import { defaultShellState, validateShellState, type ShellOperation } from '../../../shared/workspace-shell'

export const shellState = signal(defaultShellState())
export const shellError = signal('')
function accept(value: unknown): void {
  const next = validateShellState(value)
  if (next.revision >= shellState.value.revision) shellState.value = next
}
export function connectShell(): () => void {
  const off = sub('workspace:shell:state', accept)
  void inv('workspace:shell:get').then(accept).catch(() => { shellError.value = 'Workspace layout could not be loaded.' })
  return off
}
export function updateShell(operation: ShellOperation): void {
  void inv('workspace:shell:update', operation).then(value => { accept(value); shellError.value = '' })
    .catch(() => { shellError.value = 'Workspace layout could not be saved. Try again when connected.' })
}
