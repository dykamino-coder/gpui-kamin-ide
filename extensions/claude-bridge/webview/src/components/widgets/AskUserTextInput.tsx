import { useRef, useEffect } from 'preact/hooks'
import { useSignal } from '@preact/signals'

interface AskUserTextInputProps {
  placeholder: string
  value: string
  onChange: (val: string) => void
  onSubmit?: () => void
}

export function AskUserTextInput({ placeholder, value, onChange, onSubmit }: AskUserTextInputProps) {
  const focused = useSignal(false)
  const ref = useRef<HTMLTextAreaElement>(null)

  useEffect(() => {
    if (ref.current) ref.current.focus()
  }, [])

  return (
    <textarea
      ref={ref}
      placeholder={placeholder}
      value={value}
      onInput={e => onChange((e.target as HTMLTextAreaElement).value)}
      onFocus={() => { focused.value = true }}
      onBlur={() => { focused.value = false }}
      style={{
        marginTop: '8px',
        width: '100%',
        background: 'var(--bg-mantle)',
        border: `1px solid ${focused.value ? 'var(--accent-primary)' : 'var(--bg-surface)'}`,
        borderRadius: '6px',
        padding: '8px',
        color: 'var(--text-primary)',
        fontSize: '13px',
        fontFamily: 'inherit',
        resize: 'vertical',
        minHeight: '40px',
        maxHeight: '120px',
        outline: 'none',
      }}
    />
  )
}
