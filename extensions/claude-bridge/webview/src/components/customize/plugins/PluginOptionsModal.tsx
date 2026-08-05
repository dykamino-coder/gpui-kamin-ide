import type { JSX } from 'preact'
import { useEffect, useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { showToast } from '../../../signals/toasts'
import styles from './PluginOptionsModal.module.css'

interface Props {
  pluginId: string  // "name@marketplace"
  pluginName: string
  onClose: () => void
  onSaved?: () => void
}

// Mirrors `McpbUserConfigurationOption` in CLI; only the keys we actually
// surface in the form are listed — anything richer (multiple choice with
// labels, numeric ranges, regex validation) is rendered as a plain text
// input and left to plugin-side validation.
interface OptionSchema {
  type?: 'string' | 'number' | 'boolean' | 'directory' | 'file'
  description?: string
  required?: boolean
  sensitive?: boolean
  default?: unknown
  enum?: string[]
  min?: number
  max?: number
  multiple?: boolean
}

export function PluginOptionsModal({ pluginId, pluginName, onClose, onSaved }: Props): JSX.Element {
  const bridge = useBridge()
  const [schema, setSchema] = useState<Record<string, OptionSchema>>({})
  const [values, setValues] = useState<Record<string, unknown>>({})
  const [sensitiveKeys, setSensitiveKeys] = useState<Set<string>>(new Set())
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    bridge.getPluginOptionsSchema(pluginId).then(({ schema, values, sensitiveKeys }) => {
      setSchema(schema ?? {})
      // Prefill with saved non-sensitive values, then fall back to defaults
      // from the schema. Sensitive ones show as blank with a placeholder —
      // empty submit means "don't touch the existing secret".
      const merged: Record<string, unknown> = {}
      for (const [k, s] of Object.entries(schema ?? {})) {
        if (k in (values ?? {})) merged[k] = (values as any)[k]
        else if ((s as OptionSchema)?.default !== undefined) merged[k] = (s as OptionSchema).default
      }
      setValues(merged)
      setSensitiveKeys(new Set(sensitiveKeys ?? []))
      setLoading(false)
    })
  }, [pluginId])

  function updateValue(key: string, v: unknown): void {
    setValues(prev => ({ ...prev, [key]: v }))
  }

  async function handleSave(): Promise<void> {
    // Validate required fields (sensitive fields skip check if empty — means
    // keep existing secret).
    const missing: string[] = []
    for (const [k, s] of Object.entries(schema)) {
      if (!s.required) continue
      const v = values[k]
      const isSensitive = sensitiveKeys.has(k)
      const empty = v === undefined || v === null || v === '' || (Array.isArray(v) && v.length === 0)
      if (empty && !isSensitive) missing.push(k)
    }
    if (missing.length > 0) {
      showToast({ type: 'error', title: pluginName, message: `Required: ${missing.join(', ')}` })
      return
    }

    // Remove empty sensitive fields from payload — backend treats missing
    // sensitive keys as "leave untouched". Empty sensitive = user cleared
    // the field → we'd overwrite with empty → surprise secret loss.
    const payload: Record<string, unknown> = {}
    for (const [k, v] of Object.entries(values)) {
      if (sensitiveKeys.has(k) && (v === '' || v === undefined)) continue
      payload[k] = v
    }

    setSaving(true)
    try {
      await bridge.savePluginOptions(pluginId, payload)
      showToast({ type: 'success', title: pluginName, message: 'Options saved' })
      onSaved?.()
      onClose()
    } catch (err: any) {
      showToast({ type: 'error', title: pluginName, message: err?.message || String(err) })
    } finally {
      setSaving(false)
    }
  }

  function renderField(key: string, s: OptionSchema): JSX.Element {
    const v = values[key]
    const isSensitive = sensitiveKeys.has(key) || s.sensitive === true
    const placeholder =
      isSensitive && !v ? '•••••••• (saved — leave empty to keep)' :
      s.default !== undefined ? `default: ${String(s.default)}` :
      ''

    // Boolean → checkbox. Every other primitive is a one-line input. Enums
    // render as <select>. `directory` and `file` get a plain text input —
    // our renderer has no native picker here; users paste the path.
    if (s.type === 'boolean') {
      return (
        <label class={styles.checkboxRow}>
          <input
            type="checkbox"
            checked={Boolean(v)}
            onChange={(e) => updateValue(key, (e.target as HTMLInputElement).checked)}
          />
          <span>{s.description || key}</span>
        </label>
      )
    }
    if (Array.isArray(s.enum) && s.enum.length > 0) {
      return (
        <select
          class={styles.input}
          value={String(v ?? '')}
          onChange={(e) => updateValue(key, (e.target as HTMLSelectElement).value)}
        >
          <option value="">—</option>
          {s.enum.map(opt => <option key={opt} value={opt}>{opt}</option>)}
        </select>
      )
    }
    if (s.type === 'number') {
      return (
        <input
          class={styles.input}
          type="number"
          value={v as number | string | undefined ?? ''}
          placeholder={placeholder}
          min={s.min}
          max={s.max}
          onInput={(e) => {
            const raw = (e.target as HTMLInputElement).value
            updateValue(key, raw === '' ? null : Number(raw))
          }}
        />
      )
    }
    if (s.type === 'string' && s.multiple) {
      return (
        <textarea
          class={styles.input}
          value={Array.isArray(v) ? v.join('\n') : ''}
          placeholder="one value per line"
          onInput={(e) => updateValue(key, (e.target as HTMLTextAreaElement).value.split('\n').map(item => item.trim()).filter(Boolean))}
        />
      )
    }
    return (
      <input
        class={styles.input}
        type={isSensitive ? 'password' : 'text'}
        value={typeof v === 'string' || typeof v === 'number' ? String(v) : ''}
        placeholder={placeholder}
        onInput={(e) => updateValue(key, (e.target as HTMLInputElement).value)}
      />
    )
  }

  const entries = Object.entries(schema)

  return (
    <div class={styles.backdrop} onClick={onClose}>
      <div class={styles.dialog} onClick={(e) => e.stopPropagation()}>
        <div class={styles.header}>
          <h3>{pluginName} — configuration</h3>
          <button class={styles.closeBtn} onClick={onClose}><i class="fas fa-times" /></button>
        </div>
        {loading && <div class={styles.empty}>Loading schema…</div>}
        {!loading && entries.length === 0 && (
          <div class={styles.empty}>This plugin does not declare any user-configurable options.</div>
        )}
        {!loading && entries.length > 0 && (
          <div class={styles.body}>
            {entries.map(([key, s]) => (
              <div class={styles.field} key={key}>
                <label class={styles.label}>
                  <span class={styles.keyName}>
                    {key}
                    {s.required && <span class={styles.required}>*</span>}
                    {(sensitiveKeys.has(key) || s.sensitive) && <i class="fas fa-lock" title="Stored in .credentials.json" style="margin-left:6px;font-size:10px;opacity:0.6" />}
                  </span>
                  {s.description && s.type !== 'boolean' && <span class={styles.desc}>{s.description}</span>}
                </label>
                {renderField(key, s)}
              </div>
            ))}
          </div>
        )}
        <div class={styles.footer}>
          <button class={styles.cancelBtn} onClick={onClose} disabled={saving}>Cancel</button>
          <button class={styles.saveBtn} onClick={handleSave} disabled={saving || loading || entries.length === 0}>
            {saving ? 'Saving…' : 'Save'}
          </button>
        </div>
      </div>
    </div>
  )
}
