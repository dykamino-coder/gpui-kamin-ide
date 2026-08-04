import { useRef, useEffect } from 'preact/hooks'
import { useSignal } from '@preact/signals'
import type { ElicitationRequest } from '../../../shared/types'
import { activeWidgets } from '../../signals/widgets'
import { useBridge } from '../../hooks/useBridge'
import { WidgetHeader } from './WidgetHeader'
import { WidgetBody } from './WidgetBody'
import { WidgetSchemaField } from './WidgetSchemaField'
import { WidgetActions } from './WidgetActions'
import { WidgetButton } from './WidgetButton'
import styles from './ElicitationWidget.module.css'

interface ElicitationWidgetProps {
  tabId: string
  request: ElicitationRequest
}

export function ElicitationWidget({ request }: ElicitationWidgetProps) {
  const bridge = useBridge()
  const firstInputRef = useRef<HTMLInputElement | HTMLSelectElement | null>(null)

  const isAskUser = request.toolName === 'AskUserQuestion' || !request.toolName
  const isPlan = request.toolName === 'ExitPlanMode'

  const icon = isPlan ? 'fas fa-clipboard-list' : 'fas fa-circle-question'
  const title = isPlan ? 'Plan Review' : 'Claude is asking'
  const typeBadge = isPlan ? 'Plan' : 'Question'

  // Build field state from schema
  const schema = request.requestedSchema
  const properties = schema && typeof schema === 'object'
    ? ((schema as any).properties as Record<string, any> | undefined)
    : undefined

  const fieldKeys = properties ? Object.keys(properties) : []
  const fieldValues = useSignal<Record<string, string>>(
    Object.fromEntries(fieldKeys.map(k => [k, '']))
  )
  // Simple text response when no schema
  const simpleResponse = useSignal('')

  useEffect(() => {
    if (firstInputRef.current) {
      firstInputRef.current.focus()
    }
  }, [])

  function dismissCard() {
    activeWidgets.value = activeWidgets.value.filter(w => w.requestId !== request.requestId)
  }

  function handleAccept() {
    const content: Record<string, unknown> = {}
    if (fieldKeys.length > 0) {
      for (const key of fieldKeys) {
        const val = fieldValues.value[key] ?? ''
        content[key] = val === 'true' ? true : val === 'false' ? false : val
      }
    } else if (isAskUser) {
      content['response'] = simpleResponse.value
    }
    bridge.respondElicitation(request.requestId, 'accept', content)
    dismissCard()
  }

  function handleDeny() {
    bridge.respondElicitation(request.requestId, 'deny')
    dismissCard()
  }

  function handleDismiss() {
    bridge.respondElicitation(request.requestId, 'dismiss')
    dismissCard()
  }

  return (
    <div class={styles.card}>
      <WidgetHeader icon={icon} title={title} typeBadge={typeBadge} />
      <WidgetBody>
        <div class="widget-message">{request.message || 'No message provided'}</div>

        {properties && fieldKeys.map((key, i) => {
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
              onChange={val => {
                fieldValues.value = { ...fieldValues.value, [key]: val }
              }}
            />
          )
        })}

        {fieldKeys.length === 0 && isAskUser && (
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
        <WidgetButton
          variant="accept"
          label={isPlan ? 'Approve' : 'Submit'}
          onClick={handleAccept}
        />
        <WidgetButton
          variant="deny"
          label={isPlan ? 'Reject' : 'Deny'}
          onClick={handleDeny}
        />
        <WidgetButton
          variant="dismiss"
          label="Dismiss"
          onClick={handleDismiss}
        />
      </WidgetActions>
    </div>
  )
}
