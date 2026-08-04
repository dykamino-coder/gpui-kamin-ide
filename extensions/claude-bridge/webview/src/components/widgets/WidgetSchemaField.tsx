interface WidgetSchemaFieldProps {
  label: string
  type: 'text' | 'enum' | 'boolean'
  options?: string[]
  value: string
  onChange: (val: string) => void
}

export function WidgetSchemaField({ label, type, options, value, onChange }: WidgetSchemaFieldProps) {
  return (
    <div class="widget-schema-field">
      <label>{label}</label>
      {type === 'enum' && options ? (
        <select value={value} onChange={e => onChange((e.target as HTMLSelectElement).value)}>
          {options.map(opt => (
            <option key={opt} value={opt}>{opt}</option>
          ))}
        </select>
      ) : type === 'boolean' ? (
        <select value={value} onChange={e => onChange((e.target as HTMLSelectElement).value)}>
          <option value="true">Yes</option>
          <option value="false">No</option>
        </select>
      ) : (
        <input
          type="text"
          placeholder={label}
          value={value}
          onInput={e => onChange((e.target as HTMLInputElement).value)}
        />
      )}
    </div>
  )
}
