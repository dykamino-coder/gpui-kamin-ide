import 'reflect-metadata'
import { describe, expect, it } from 'vitest'
import { tierForModel } from './users'

describe('dashboard model prices', () => {
  it('uses Fable 5.1 pricing without repricing old session models', () => {
    expect(tierForModel('claude-fable-5-1')).toEqual({ input: 10, output: 50, cacheWrite: 12.5, cacheRead: 0.25 })
    expect(tierForModel('claude-opus-5')).toEqual({ input: 5, output: 25, cacheWrite: 6.25, cacheRead: 0.5 })
  })

  it('prices current Sonnet models without changing historical Sonnet rates', () => {
    expect(tierForModel('claude-sonnet-5-5')).toEqual({ input: 2, output: 10, cacheWrite: 2.5, cacheRead: 0.2 })
    expect(tierForModel('claude-sonnet-5')).toEqual({ input: 2, output: 10, cacheWrite: 2.5, cacheRead: 0.2 })
    expect(tierForModel('claude-sonnet-4-6')).toEqual({ input: 3, output: 15, cacheWrite: 3.75, cacheRead: 0.3 })
  })
})
