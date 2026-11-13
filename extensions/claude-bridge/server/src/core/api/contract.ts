import type { z } from 'zod'

export interface Contract {
  method: string
  path: string
  summary: string
  tag: string
  description?: string
  body?: z.ZodType
  bodyOptional?: boolean
  media?: string
  query?: z.ZodObject
  response?: z.ZodType
  responseMedia?: string
  statuses?: number[]
  /** Undefined = dashboard Basic/session or scoped API bearer; [] = public. */
  security?: string[]
}

export function operation(
  method: string,
  path: string,
  summary: string,
  response?: z.ZodType,
  extra: Partial<Contract> = {},
): Contract {
  return { method, path, summary, response, tag: 'Dashboard', ...extra }
}
