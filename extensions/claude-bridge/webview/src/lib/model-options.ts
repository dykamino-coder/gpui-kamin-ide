// Models offered for new selections. A resumed session can still use a model
// absent from this list; never label it as the default or change it implicitly.
export const DEFAULT_MODEL_ID = 'claude-opus-5-5'

export interface ModelOption {
  value: string
  icon: string
  name: string
  description: string
}

export const MODEL_OPTIONS: readonly ModelOption[] = [
  { value: 'claude-opus-5-5', icon: 'fa-gem', name: 'Opus 5.5', description: 'Default for new sessions, 1M-token context' },
  { value: 'claude-opus-5', icon: 'fa-gem', name: 'Opus 5', description: 'Previous Opus version, 1M-token context' },
  { value: 'claude-sonnet-5', icon: 'fa-feather', name: 'Sonnet 5', description: 'Fast and balanced' },
  { value: 'claude-haiku-4-5', icon: 'fa-bolt', name: 'Haiku 4.5', description: 'Fastest, lightweight tasks' },
  { value: 'claude-fable-5-1', icon: 'fa-book-open', name: 'Fable 5.1', description: 'Complex, long-running work; access depends on account' },
]

export function selectedModelOption(modelId: string | null | undefined): ModelOption {
  const id = modelId || DEFAULT_MODEL_ID
  return MODEL_OPTIONS.find(option => option.value === id) ?? {
    value: id,
    icon: 'fa-circle-question',
    name: id,
    description: 'Current session model; select another model to change it',
  }
}
