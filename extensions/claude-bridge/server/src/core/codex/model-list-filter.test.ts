import { describe, expect, it } from 'vitest'
import { currentModels, modelGeneration, type ListedModel } from './model-list-filter'

const m = (slug: string, extra: Partial<ListedModel> = {}): ListedModel => ({
  id: slug,
  model: slug,
  hidden: false,
  upgrade: null,
  upgradeInfo: null,
  isDefault: false,
  ...extra,
})
const slugs = (list: ListedModel[]) => list.map((x) => x.model)

describe('GPT model list: current generation only', () => {
  it('parses gpt-<major>[.<minor>][-<tier>] slugs', () => {
    expect(modelGeneration('gpt-6.1-sol')).toEqual({ major: 6, minor: 1, tier: 'sol' })
    expect(modelGeneration('gpt-6-Astra')).toEqual({ major: 6, minor: 0, tier: 'astra' })
    expect(modelGeneration('gpt-5.5')).toEqual({ major: 5, minor: 5, tier: '' })
    expect(modelGeneration('codex-auto-review')).toBeNull()
    expect(modelGeneration('gpt-daybreak-blue-latest')).toBeNull()
  })

  it('keeps the newest major and the newest minor of each tier (codex 0.160.1 catalog)', () => {
    const list = [
      m('gpt-6-astra'),
      m('gpt-6.1-sol', { isDefault: true }),
      m('gpt-6-sol'),
      m('gpt-6-luna'),
      m('gpt-5.6-sol', { upgrade: 'gpt-6-sol' }),
      m('gpt-5.6-terra', { upgrade: 'gpt-6-sol' }),
      m('gpt-5.6-luna', { upgrade: 'gpt-6-luna' }),
      m('gpt-daybreak-blue-latest', { hidden: true }),
      m('gpt-5.5', { upgrade: 'gpt-6-sol' }),
      m('codex-auto-review', { hidden: true }),
    ]
    const kept = currentModels(list)
    expect(slugs(kept)).toEqual(['gpt-6-astra', 'gpt-6.1-sol', 'gpt-6-luna'])
    expect(kept.filter((x) => x.isDefault).map((x) => x.model)).toEqual(['gpt-6.1-sol'])
  })

  it('drops a model superseded through upgrade or upgradeInfo even in the newest generation', () => {
    const kept = currentModels([
      m('gpt-7-nova', { upgradeInfo: { model: 'gpt-7-nova-2' } }),
      m('gpt-7-nova-2'),
      m('gpt-7-orbit', { upgrade: 'gpt-9-gone' }),
    ])
    // gpt-7-nova-2 is not a tier slug Bridge can place, so it stays as is;
    // an upgrade to a model that is not listed does not hide anything.
    expect(slugs(kept)).toEqual(['gpt-7-nova-2', 'gpt-7-orbit'])
  })

  it('moves the default to its upgrade, else to the first kept model, when the default is dropped', () => {
    const viaUpgrade = currentModels([
      m('gpt-6-luna'),
      m('gpt-6-sol'),
      m('gpt-5.5', { isDefault: true, upgrade: 'gpt-6-sol' }),
    ])
    expect(viaUpgrade.filter((x) => x.isDefault).map((x) => x.model)).toEqual(['gpt-6-sol'])
    const first = currentModels([m('gpt-6-luna'), m('gpt-5.6-terra', { isDefault: true })])
    expect(first.filter((x) => x.isDefault).map((x) => x.model)).toEqual(['gpt-6-luna'])
  })

  it('keeps slugs outside the gpt-N scheme and a list without any', () => {
    expect(slugs(currentModels([m('o9-preview'), m('gpt-6-luna'), m('gpt-5-luna')]))).toEqual([
      'o9-preview',
      'gpt-6-luna',
    ])
    expect(slugs(currentModels([m('gpt-fake', { isDefault: true })]))).toEqual(['gpt-fake'])
  })
})
