import type { JSX } from 'preact'
import { useEffect, useRef } from 'preact/hooks'
import type { SlashCommand } from '../chat-header/slash-commands'
import { SlashItem } from './SlashItem'

interface SlashAutocompleteProps {
  query: string
  matches: SlashCommand[]
  selectedIndex: number
  visible: boolean
  onSelect: (cmd: string) => void
  onHover: (index: number) => void
}

export function SlashAutocomplete({ visible, matches, selectedIndex, onSelect, onHover }: SlashAutocompleteProps): JSX.Element {
  const containerRef = useRef<HTMLDivElement>(null)

  // Keep the highlighted row inside the visible window when the user
  // arrow-navigates past the overflow edge. `block: 'nearest'` snaps the
  // selection just into view without yanking the whole list on every step.
  useEffect(() => {
    if (!visible || selectedIndex < 0) return
    const container = containerRef.current
    if (!container) return
    const target = container.children[selectedIndex] as HTMLElement | undefined
    if (!target) return
    const top = target.offsetTop
    const bottom = top + target.offsetHeight
    const viewTop = container.scrollTop
    const viewBottom = viewTop + container.clientHeight
    if (top < viewTop) container.scrollTop = top
    else if (bottom > viewBottom) container.scrollTop = bottom - container.clientHeight
  }, [selectedIndex, visible, matches.length])

  return (
    <div ref={containerRef} class={`slash-autocomplete${visible ? ' visible' : ''}`} id="slash-autocomplete">
      {matches.map((cmd, i) => (
        <SlashItem
          key={cmd.name}
          name={cmd.name}
          description={cmd.description}
          isSelected={i === selectedIndex}
          onClick={() => onSelect(cmd.name === '/model' ? cmd.name + ' ' : cmd.name)}
          onMouseEnter={() => onHover(i)}
        />
      ))}
    </div>
  )
}
