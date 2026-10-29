import { signal } from '@preact/signals'
import type { CommandSnapshot } from '../../../shared/commands'
export const commandSnapshots = signal<CommandSnapshot[]>([])
export const commandOutput = signal<{ sessionId: string; id: string; output: string } | null>(null)
