import { AskUserOption } from './AskUserOption'

interface AskUserOptionsProps {
  options: string[]
  requestId: string
  selectedIndex: number | null
  /** multiSelect: чекбокс-семантика — onChange тогглит индекс. */
  multiSelect?: boolean
  pickedIndexes?: number[]
  onChange: (index: number) => void
}

export function AskUserOptions({ options, requestId, selectedIndex, multiSelect, pickedIndexes, onChange }: AskUserOptionsProps) {
  return (
    <div style="margin-top:8px;display:flex;flex-direction:column;gap:4px;">
      {options.map((opt, idx) => (
        <AskUserOption
          key={idx}
          label={opt}
          // Радио-группировка браузера снимала бы прочие отметки — в
          // multiSelect у каждой опции своё имя.
          name={multiSelect ? `ask-${requestId}-${idx}` : `ask-${requestId}`}
          value={idx}
          kind={multiSelect ? 'checkbox' : 'radio'}
          checked={multiSelect ? (pickedIndexes ?? []).includes(idx) : selectedIndex === idx}
          onChange={() => onChange(idx)}
        />
      ))}
    </div>
  )
}
