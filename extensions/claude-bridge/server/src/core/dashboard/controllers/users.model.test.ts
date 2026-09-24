import 'reflect-metadata'
import { describe, expect, it } from 'vitest'
import { tierForModel } from './users'

describe('dashboard model prices', () => {
  it('uses Fable 5.1 pricing without repricing old session models', () => {
    expect(tierForModel('claude-fable-5-1')).toEqual({ input: 10, output: 50, cacheWrite: 12.5, cacheRead: 0.25 })
    expect(tierForModel('claude-opus-5')).toEqual({ input: 5, output: 25, cacheWrite: 6.25, cacheRead: 0.5 })
  })
})
