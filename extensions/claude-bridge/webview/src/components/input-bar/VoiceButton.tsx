import type { JSX } from 'preact'
import type { VoiceInputState } from '../../hooks/useVoiceInput'

interface VoiceButtonProps {
  voiceState: VoiceInputState
  onToggle: () => void
}

export function VoiceButton({ voiceState, onToggle }: VoiceButtonProps): JSX.Element | null {
  if (!voiceState.visible) return null

  const { state, countdownDigit } = voiceState

  let stateClass = ''
  let tooltip = 'Voice input'
  let content: JSX.Element

  if (state === 'countdown') {
    stateClass = 'countdown'
    tooltip = `Wait ${countdownDigit}...`
    content = <span>{countdownDigit}</span>
  } else if (state === 'recording') {
    stateClass = 'recording'
    tooltip = 'Stop recording'
    content = <i class="fas fa-stop" />
  } else if (state === 'processing') {
    stateClass = 'processing'
    tooltip = 'Processing...'
    content = <i class="fas fa-spinner fa-spin" />
  } else {
    content = <i class="fas fa-microphone" />
  }

  return (
    <button
      class={`attach-btn voice-btn${stateClass ? ` ${stateClass}` : ''}`}
      data-tooltip={tooltip}
      type="button"
      onClick={state !== 'processing' ? onToggle : undefined}
    >
      {content}
    </button>
  )
}
