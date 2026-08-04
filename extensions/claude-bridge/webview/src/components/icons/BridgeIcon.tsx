import type { JSX } from 'preact'

interface BridgeIconProps {
  size?: number
  color?: string
  class?: string
  style?: string
}

export function BridgeIcon({ size = 16, color = 'currentColor', class: className, style }: BridgeIconProps): JSX.Element {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 512 512"
      width={size}
      height={size}
      class={className}
      style={style}
    >
      <g style={`fill:${color};stroke:${color}`}>
        <rect x="32" y="384" width="448" height="48" rx="16" stroke="none" />
        <rect x="112" y="432" width="32" height="64" stroke="none" />
        <rect x="368" y="432" width="32" height="64" stroke="none" />
        <ellipse cx="256" cy="272" rx="120" ry="76" stroke="none" />
        <circle cx="216" cy="160" r="16" stroke="none" />
        <rect x="204" y="160" width="24" height="48" stroke="none" />
        <circle cx="296" cy="160" r="16" stroke="none" />
        <rect x="284" y="160" width="24" height="48" stroke="none" />
        <path d="M120,136 c-50,0-70-50-24-90 c20-16,50-16,70-4 c-30,12-40,40-40,70 c20-12,40-10,60,0 C196,124,170,136,120,136 z" stroke="none" />
        <path d="M392,136 c50,0,70-50,24-90 c-20-16-50-16-70-4 c30,12,40,40,40,70 c-20-12-40-10-60,0 C316,124,342,136,392,136 z" stroke="none" />
        <g stroke-width="28" stroke-linecap="round" fill="none">
          <path d="M 140 260 C 80 260 64 320 64 360" />
          <path d="M 150 300 C 100 310 96 360 96 384" />
          <path d="M 372 260 C 432 260 448 320 448 360" />
          <path d="M 362 300 C 412 310 416 360 416 384" />
          <path d="M 176 224 C 120 190 120 150 120 150" />
          <path d="M 336 224 C 392 190 392 150 392 150" />
        </g>
      </g>
    </svg>
  )
}
