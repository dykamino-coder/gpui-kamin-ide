// The effort selector names the level the chat's turns run with. A ChatGPT
// chat without a known level used to show High (the Claude fallback) while its
// turns went without effort (REAL-121).
import { expect, it } from 'vitest'
import { codexEffortOptions, effortShown } from './EffortDropdown'

it('shows the level a ChatGPT chat reports, and Default while it reports none', () => {
  expect(effortShown({ engine: 'codex', effort: 'medium' }, 'high').value).toBe('medium')
  expect(effortShown({ engine: 'codex', effort: null }, 'high').value).toBe('default')
  expect(effortShown({ engine: 'codex' }, 'high').name).toBe('Default')
})

it('keeps High for a Claude chat without a level: its CLI starts with --effort high', () => {
  expect(effortShown({ engine: 'claude' }, undefined).value).toBe('high')
  expect(effortShown(undefined, 'low').value).toBe('low')
  expect(effortShown({ effort: 'max' }, 'low').value).toBe('max')
})

it('lists the levels advertised by each Codex model and shows its native selection', () => {
  expect(codexEffortOptions(['low', 'medium']).map(option => option.value)).toEqual(['low', 'medium'])
  expect(codexEffortOptions(['xhigh', 'max', 'ultra']).map(option => option.name)).toEqual(['Extra high', 'Max', 'Ultra'])
  expect(effortShown({ engine: 'codex', effort: 'ultra' }, null).value).toBe('ultra')
})
