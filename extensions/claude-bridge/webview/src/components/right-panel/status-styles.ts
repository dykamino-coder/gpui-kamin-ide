// Shared status-styles map for both PlanList (5 statuses) and TodoList
// (3 statuses). Centralizes icon + accent + bg + label so the visual
// rhythm stays identical between the two panels.

export type Status = 'pending' | 'in_progress' | 'completed' | 'cancelled' | 'failed'

export interface StatusStyle {
  icon: string
  accent: string   // CSS var() for the accent line + label color
  text: string     // CSS var() for the body text color
  bg: string       // background — kept as rgba() string since most are alpha tints
  strike: boolean
  label: string
  glow?: string    // optional box-shadow var() for the active-row halo
}

export const STATUS_STYLES: Record<Status, StatusStyle> = {
  pending: {
    icon: 'fa-regular fa-square',
    accent: 'var(--text-muted)',
    text: 'var(--text-secondary)',
    bg: 'rgba(108,112,134,0.06)',
    strike: false,
    label: 'todo',
  },
  in_progress: {
    icon: 'fas fa-circle-half-stroke',
    accent: 'var(--accent-yellow)',
    text: 'var(--text-primary)',
    bg: 'rgba(249,226,175,0.10)',
    strike: false,
    label: 'now',
    // Subtle 1px ring — was a 14px-blur neon glow which read as AI-slop
    // "dark mode with coloured shadow halos". Keeping just the ring keeps
    // the visual cue (this row is the active step) without the noise.
    glow: 'inset 0 0 0 1px rgba(249,226,175,0.35)',
  },
  completed: {
    icon: 'fa-regular fa-square-check',
    accent: 'var(--accent-green)',
    text: 'var(--text-muted)',
    bg: 'rgba(166,227,161,0.05)',
    strike: true,
    label: 'done',
  },
  cancelled: {
    icon: 'fas fa-ban',
    accent: 'var(--accent-red)',
    text: 'var(--text-muted)',
    bg: 'rgba(243,139,168,0.06)',
    strike: true,
    label: 'skip',
  },
  failed: {
    icon: 'fas fa-triangle-exclamation',
    accent: 'var(--accent-red)',
    text: 'var(--text-primary)',
    bg: 'rgba(243,139,168,0.10)',
    strike: false,
    label: 'fail',
  },
}
