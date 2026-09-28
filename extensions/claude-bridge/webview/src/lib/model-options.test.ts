import { describe, expect, it } from 'vitest'
import { MODEL_OPTIONS, selectedModelOption } from './model-options'

describe('model picker choices', () => {
  it('offers Sonnet 5.5 and retains previous choices with Opus 5.5 as the default', () => {
    expect(MODEL_OPTIONS.map(option => option.value)).toEqual([
      'claude-opus-5-5',
      'claude-opus-5',
      'claude-sonnet-5-5',
      'claude-sonnet-5',
      'claude-haiku-4-5',
      'claude-fable-5-1',
    ])
    expect(selectedModelOption(undefined).value).toBe('claude-opus-5-5')
  })

  it('shows an older session model accurately without replacing it', () => {
    expect(selectedModelOption('claude-fable-5')).toMatchObject({
      value: 'claude-fable-5',
      name: 'claude-fable-5',
    })
    expect(selectedModelOption('haiku').value).toBe('haiku')
  })
})
