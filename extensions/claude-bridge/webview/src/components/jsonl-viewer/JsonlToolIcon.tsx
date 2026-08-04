import type { JSX } from 'preact'

export const TOOL_ICONS: Record<string, string> = {
  Bash: 'fa-terminal',
  PowerShell: 'fa-terminal',
  Read: 'fa-file-lines',
  Write: 'fa-pen',
  Edit: 'fa-pen-to-square',
  Glob: 'fa-magnifying-glass',
  Grep: 'fa-search',
  WebFetch: 'fa-globe',
  WebSearch: 'fa-globe',
  Agent: 'fa-robot',
  TodoWrite: 'fa-check',
  NotebookEdit: 'fa-book',
  TeamCreate: 'fa-users',
  TeamDelete: 'fa-user-xmark',
  SendMessage: 'fa-paper-plane',
  ToolSearch: 'fa-screwdriver-wrench',
  Skill: 'fa-wand-magic-sparkles',
  TaskCreate: 'fa-list-check',
  TaskUpdate: 'fa-list-check',
  AskUserQuestion: 'fa-circle-question',
  ExitPlanMode: 'fa-clipboard-check',
  ShowWidget: 'fa-chart-simple',
  LspDiagnostics: 'fa-stethoscope',
}

interface JsonlToolIconProps {
  toolName: string
}

export function JsonlToolIcon({ toolName }: JsonlToolIconProps): JSX.Element {
  const iconClass = TOOL_ICONS[toolName] ?? 'fa-wrench'
  return <i class={`fas ${iconClass}`} />
}
