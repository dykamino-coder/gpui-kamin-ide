import { describe, expect, it } from 'vitest'
import {
  MODEL_OPTIONS,
  engineOfModel,
  gptModelOptions,
  planModelSelection,
  selectedGptOption,
  selectedModelOption,
} from './model-options'

describe('model picker choices', () => {
  it('offers only current versions, with Opus 5.5 as the default', () => {
    expect(MODEL_OPTIONS.map(option => option.value)).toEqual([
      'claude-opus-5-5',
      'claude-sonnet-5-5',
      'claude-haiku-5-5',
      'claude-fable-5-2',
    ])
    expect(selectedModelOption(undefined).value).toBe('claude-opus-5-5')
  })

  it('keeps a session already running on a superseded model', () => {
    // Убранный из списка выбор не стирает модель идущей сессии: она
    // показывается как есть и молча не подменяется.
    expect(selectedModelOption('claude-opus-5')).toMatchObject({
      value: 'claude-opus-5',
      name: 'claude-opus-5',
    })
    expect(selectedModelOption('claude-sonnet-5').value).toBe('claude-sonnet-5')
    expect(selectedModelOption('claude-fable-5-1')).toMatchObject({ value: 'claude-fable-5-1', name: 'claude-fable-5-1' })
  })

  it('shows an older session model accurately without replacing it', () => {
    expect(selectedModelOption('claude-fable-5')).toMatchObject({
      value: 'claude-fable-5',
      name: 'claude-fable-5',
    })
    expect(selectedModelOption('haiku').value).toBe('haiku')
  })
})

describe('GPT models in the picker', () => {
  const models = [
    { model: 'gpt-5.5', displayName: 'GPT-5.5', description: 'Frontier', isDefault: true },
    { model: 'gpt-5.4-mini', displayName: '', description: '', isDefault: false },
    { model: '', displayName: 'broken', description: '', isDefault: false },
  ]

  it('lists the server GPT models by display name and skips empty ids', () => {
    expect(gptModelOptions(models).map(o => [o.value, o.name])).toEqual([
      ['gpt-5.5', 'GPT-5.5'],
      ['gpt-5.4-mini', 'gpt-5.4-mini'],
    ])
  })

  it('tells Claude ids from GPT ids', () => {
    expect(engineOfModel('claude-opus-5-5')).toBe('claude')
    expect(engineOfModel('gpt-5.5')).toBe('codex')
  })

  it('changes the model inside the same agent and opens a new chat across agents', () => {
    expect(planModelSelection('claude', 'claude-sonnet-5-5')).toEqual({ kind: 'change', model: 'claude-sonnet-5-5' })
    expect(planModelSelection('claude', 'gpt-5.5')).toEqual({ kind: 'switch', engine: 'codex', model: 'gpt-5.5' })
    expect(planModelSelection('codex', 'gpt-5.4-mini')).toEqual({ kind: 'change', model: 'gpt-5.4-mini' })
    expect(planModelSelection('codex', 'claude-opus-5-5')).toEqual({ kind: 'switch', engine: 'claude', model: 'claude-opus-5-5' })
  })

  it('without a tab remembers a Claude pick and still opens a GPT chat', () => {
    expect(planModelSelection(null, 'claude-haiku-5-5')).toEqual({ kind: 'remember', model: 'claude-haiku-5-5' })
    expect(planModelSelection(null, 'gpt-5.5')).toEqual({ kind: 'new-chat', engine: 'codex', model: 'gpt-5.5' })
  })

  it('shows a codex chat model that is not in the list as-is', () => {
    const gpt = gptModelOptions(models)
    expect(selectedGptOption('gpt-5.5', gpt).name).toBe('GPT-5.5')
    expect(selectedGptOption('gpt-4.1', gpt)).toMatchObject({ value: 'gpt-4.1', name: 'gpt-4.1' })
    expect(selectedGptOption(undefined, gpt).name).toBe('ChatGPT')
  })
})