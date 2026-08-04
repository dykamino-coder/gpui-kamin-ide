// Server-initiated MCP elicitation (`elicitation/create`) rendered as a
// NON-BLOCKING widget. Previously this used native `window.prompt`, which froze
// the whole renderer thread until dismissed. Reuses the same schema-form
// primitives as the CLI ElicitationWidget but responds over the MCP channel
// (`respondMcpElicitation`, action accept|cancel) instead of respondElicitation.
import { useRef, useEffect } from 'preact/hooks'
import { useSignal } from '@preact/signals'
import { activeWidgets } from '../../signals/widgets'
import { useBridge } from '../../hooks/useBridge'
import { WidgetHeader } from './WidgetHeader'
import { WidgetBody } from './WidgetBody'
import { WidgetSchemaField } from './WidgetSchemaField'
import { WidgetActions } from './WidgetActions'
import { WidgetButton } from './WidgetButton'
import styles from './ElicitationWidget.module.css'

export interface McpElicitationData {
  requestId: string
  serverName?: string
  message?: string
  requestedSchema?: unknown
}

export function McpElicitationWidget({ data }: { data: McpElicitationData }) {
  const bridge = useBridge()
  const firstInputRef = useRef<HTMLInputElement | HTMLSelectElement | null>(null)

  const schema = data.requestedSchema
  const properties = schema && typeof schema === 'object'
    ? ((schema as any).properties as Record<string, any> | undefined)
    : undefined
  const fieldKeys = properties ? Object.keys(properties) : []
  const fieldValues = useSignal<Record<string, string>>(
    Object.fromEntries(fieldKeys.map(k => [k, '']))
  )
  const simpleResponse = useSignal('')

  useEffect(() => { firstInputRef.current?.focus() }, [])

  function dismissCard() {
    activeWidgets.value = activeWidgets.value.filter(w => w.requestId !== data.requestId)
  }

  function handleAccept() {
    const content: Record<string, unknown> = {}
    if (fieldKeys.length > 0) {
      for (const key of fieldKeys) {
        const val = fieldValues.value[key] ?? ''
        content[key] = val === 'true' ? true : val === 'false' ? false : val
      }
    } else {
      // No schema → free-text under a conventional `answer` key (mirrors the
      // former prompt's JSON-or-string fallback).
      content['answer'] = simpleResponse.value
    }
    bridge.respondMcpElicitation(data.requestId, { action: 'accept', content })
    dismissCard()
  }

  function handleCancel() {
    bridge.respondMcpElicitation(data.requestId, { action: 'cancel' })
    dismissCard()
  }

  const title = data.serverName ? `MCP: ${data.serverName}` : 'MCP server request'

  return (
    <div class={styles.card}>
      <WidgetHeader icon="fas fa-plug" title={title} typeBadge="Input" />
      <WidgetBody>
        <div class="widget-message">{data.message || 'The MCP server is requesting input'}</div>

        {properties && fieldKeys.map((key) => {
          const propSchema = properties[key]
          let fieldType: 'text' | 'enum' | 'boolean' = 'text'
          let options: string[] | undefined
          if (propSchema.enum && Array.isArray(propSchema.enum)) {
            fieldType = 'enum'
            options = propSchema.enum.map(String)
          } else if (propSchema.type === 'boolean') {
            fieldType = 'boolean'
          }
          return (
            <WidgetSchemaField
              key={key}
              label={propSchema.description || key}
              type={fieldType}
              options={options}
              value={fieldValues.value[key] ?? ''}
              onChange={val => { fieldValues.value = { ...fieldValues.value, [key]: val } }}
            />
          )
        })}

        {fieldKeys.length === 0 && (
          <div class="widget-schema-field">
            <input
              ref={firstInputRef as any}
              type="text"
              placeholder="Type your response..."
              value={simpleResponse.value}
              onInput={e => { simpleResponse.value = (e.target as HTMLInputElement).value }}
              onKeyDown={e => { if (e.key === 'Enter') handleAccept() }}
            />
          </div>
        )}
      </WidgetBody>
      <WidgetActions>
        <WidgetButton variant="accept" label="Submit" onClick={handleAccept} />
        <WidgetButton variant="dismiss" label="Cancel" onClick={handleCancel} />
      </WidgetActions>
    </div>
  )
}
