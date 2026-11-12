// Models offered for new selections. A resumed session can still use a model
// absent from this list; never label it as the default or change it implicitly.
//
// Superseded versions are not offered: Opus 5.5, Sonnet 5.5 and Fable 5.2
// replace Opus 5, Sonnet 5 and Fable 5.1 outright, so listing both only asks the user to pick between a
// model and its own older build. Sessions already running on a removed model
// keep it — `selectedModelOption` renders any unknown id as-is.
export const DEFAULT_MODEL_ID = 'claude-opus-5-5'

export interface ModelOption {
  value: string
  icon: string
  name: string
  description: string
}

export const MODEL_OPTIONS: readonly ModelOption[] = [
  { value: 'claude-opus-5-5', icon: 'fa-gem', name: 'Opus 5.5', description: 'Default for new sessions, 1M-token context' },
  { value: 'claude-sonnet-5-5', icon: 'fa-feather', name: 'Sonnet 5.5', description: 'Latest Sonnet, 1M-token context' },
  { value: 'claude-haiku-5-5', icon: 'fa-bolt', name: 'Haiku 5.5', description: 'Fastest, 1M-token context' },
  { value: 'claude-fable-5-2', icon: 'fa-book-open', name: 'Fable 5.2', description: 'Complex, long-running work, 1M-token context; access depends on account' },
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

// ─── GPT (ChatGPT through Codex CLI) ─────────────────────────────────────────
// A chat keeps its agent for life: picking a model of the same agent changes
// the model in place; a model of the other agent opens a new chat on it.

export type ModelEngine = 'claude' | 'codex'

/** Same shape as the server's codex `model/list` entry (subset used here). */
export interface GptModel {
  model: string
  displayName: string
  description: string
  isDefault: boolean
}

export const GPT_ICON = 'fa-robot'

export function gptModelOptions(models: readonly GptModel[]): ModelOption[] {
  return models
    .filter(m => typeof m.model === 'string' && m.model)
    .map(m => ({
      value: m.model,
      icon: GPT_ICON,
      name: m.displayName || m.model,
      description: m.description || (m.isDefault ? 'Default ChatGPT model' : 'ChatGPT model'),
    }))
}

/** A Claude model id is the only thing that marks a model as Claude's. */
export function engineOfModel(modelId: string): ModelEngine {
  return /^claude/i.test(modelId) ? 'claude' : 'codex'
}

export type ModelSelection =
  | { kind: 'change'; model: string }
  | { kind: 'new-chat'; engine: ModelEngine; model: string }
  | { kind: 'remember'; model: string }

/** What picking `model` does in a tab running `tabEngine` (no tab → remember
 *  the Claude pick for the next chat; a GPT pick still opens a GPT chat). */
export function planModelSelection(tabEngine: ModelEngine | null, model: string): ModelSelection {
  const engine = engineOfModel(model)
  if (tabEngine === null) return engine === 'claude' ? { kind: 'remember', model } : { kind: 'new-chat', engine, model }
  return engine === tabEngine ? { kind: 'change', model } : { kind: 'new-chat', engine, model }
}

/** The selected entry of a codex chat: its GPT model, or the id as-is. */
export function selectedGptOption(modelId: string | null | undefined, gpt: readonly ModelOption[]): ModelOption {
  const found = modelId ? gpt.find(option => option.value === modelId) : undefined
  return found ?? {
    value: modelId || '',
    icon: GPT_ICON,
    name: modelId || 'ChatGPT',
    description: modelId ? 'Current GPT model' : 'ChatGPT default model',
  }
}