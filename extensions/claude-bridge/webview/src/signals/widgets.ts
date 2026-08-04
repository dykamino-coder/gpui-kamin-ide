import { signal } from '@preact/signals'

export interface ActiveWidget {
  requestId: string
  resolved: boolean
  type: 'elicitation' | 'permission' | 'askUser' | 'mcpElicitation'
  data: any
}

export const activeWidgets = signal<ActiveWidget[]>([])
