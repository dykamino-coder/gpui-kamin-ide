import { useRef } from 'preact/hooks'
import { signal } from '@preact/signals'
import styles from './CodeBlock.module.css'

const copiedKey = signal<string | null>(null)
let nextId = 0

function copyToClipboard(text: string, key: string) {
  const fallback = () => {
    const ta = document.createElement('textarea')
    ta.value = text
    ta.style.cssText = 'position:fixed;opacity:0;left:-9999px'
    document.body.appendChild(ta)
    ta.focus()
    ta.select()
    try { document.execCommand('copy') } catch {}
    document.body.removeChild(ta)
  }
  if (navigator.clipboard?.writeText) {
    navigator.clipboard.writeText(text).catch(fallback)
  } else {
    fallback()
  }
  copiedKey.value = key
  setTimeout(() => { if (copiedKey.value === key) copiedKey.value = null }, 1500)
}

interface CodeBlockProps {
  code: string
  copyCode?: string  // if provided, this is copied instead of code (one-liner vs display)
  label?: string
}

export function CodeBlock({ code, copyCode, label }: CodeBlockProps) {
  const idRef = useRef(`cb-${nextId++}`)
  const key = idRef.current
  const isCopied = copiedKey.value === key

  return (
    <div class={`${styles.block} ${label ? styles.hasLabel : ''}`}>
      {label && <div class={styles.label}>{label}</div>}
      <pre class={styles.code}>{code}</pre>
      <button
        class={`${styles.copyBtn} ${isCopied ? styles.copied : ''}`}
        onClick={() => copyToClipboard(copyCode ?? code, key)}
        title="Copy"
      >
        <i class={`fa-solid ${isCopied ? 'fa-check' : 'fa-copy'}`} />
      </button>
    </div>
  )
}
