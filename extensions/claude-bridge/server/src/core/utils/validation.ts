// ============================================================================
// Request Validation
// ============================================================================

import type { AnthropicRequest, AnthropicMessage, ThinkingConfig } from "../types"

export interface ValidationResult {
  valid: boolean
  error?: string
  data?: AnthropicRequest
}

function isValidContentBlock(block: unknown): block is Record<string, unknown> {
  if (typeof block !== "object" || block === null) return false
  const b = block as Record<string, unknown>

  // Must have a type field
  if (typeof b.type !== "string") return false

  // Supported content block types (including redacted_thinking for extended thinking)
  const validTypes = ["text", "image", "tool_use", "tool_result", "thinking", "redacted_thinking"]
  if (!validTypes.includes(b.type)) return false

  // Validate text block
  if (b.type === "text" && typeof b.text !== "string") return false

  // Validate thinking block (extended thinking / reasoning)
  if (b.type === "thinking") {
    // thinking can be string or undefined (e.g., empty thinking with just a signature)
    if (typeof b.thinking !== "string" && b.thinking !== undefined && b.thinking !== null) return false
  }

  // Validate redacted_thinking block (opaque, must be echoed back as-is)
  // Only requires 'data' field (base64 string)
  if (b.type === "redacted_thinking") {
    if (typeof b.data !== "string") return false
  }

  // Validate tool_use block
  if (b.type === "tool_use") {
    if (typeof b.id !== "string") return false
    if (typeof b.name !== "string") return false
  }

  // Validate tool_result block
  if (b.type === "tool_result") {
    if (typeof b.tool_use_id !== "string") return false
  }

  return true
}

function isValidMessage(msg: unknown): msg is AnthropicMessage {
  if (typeof msg !== "object" || msg === null) return false
  const m = msg as Record<string, unknown>

  if (m.role !== "user" && m.role !== "assistant") return false

  if (typeof m.content === "string") return true

  if (Array.isArray(m.content)) {
    return m.content.every(isValidContentBlock)
  }

  return false
}

export function validateRequest(body: unknown): ValidationResult {
  if (typeof body !== "object" || body === null) {
    return { valid: false, error: "Request body must be an object" }
  }

  const req = body as Record<string, unknown>

  // messages is required
  if (!Array.isArray(req.messages)) {
    return { valid: false, error: "messages must be an array" }
  }

  if (req.messages.length === 0) {
    return { valid: false, error: "messages cannot be empty" }
  }

  for (let i = 0; i < req.messages.length; i++) {
    if (!isValidMessage(req.messages[i])) {
      // Detailed diagnostics: which block failed?
      const msg = req.messages[i] as Record<string, unknown> | null
      let detail = ""
      if (msg && typeof msg === "object") {
        detail = ` (role=${msg.role}`
        if (Array.isArray(msg.content)) {
          const types = msg.content.map((b: any) => {
            if (typeof b !== "object" || b === null) return `non-object(${typeof b})`
            const bt = (b as Record<string, unknown>).type
            return typeof bt === "string" ? bt : `no-type(${typeof bt})`
          })
          detail += `, contentTypes=[${types.join(",")}]`
          // Find the first invalid block
          for (let j = 0; j < msg.content.length; j++) {
            if (!isValidContentBlock(msg.content[j])) {
              const badBlock = msg.content[j]
              detail += `, failedBlock=${j}:${JSON.stringify(badBlock).slice(0, 300)}`
              break
            }
          }
        } else {
          detail += `, content=${typeof msg.content}`
        }
        detail += ")"
      }
      return { valid: false, error: `Invalid message at index ${i}${detail}` }
    }
  }

  // model is optional but must be string if provided
  if (req.model !== undefined && typeof req.model !== "string") {
    return { valid: false, error: "model must be a string" }
  }

  // stream is optional but must be boolean if provided
  if (req.stream !== undefined && typeof req.stream !== "boolean") {
    return { valid: false, error: "stream must be a boolean" }
  }

  // max_tokens is optional but must be number if provided
  if (req.max_tokens !== undefined && typeof req.max_tokens !== "number") {
    return { valid: false, error: "max_tokens must be a number" }
  }

  // system is optional, can be string or array of content blocks
  let systemPrompt: string | undefined
  if (req.system !== undefined) {
    if (typeof req.system === "string") {
      systemPrompt = req.system
    } else if (Array.isArray(req.system)) {
      // Extract text from content blocks, skipping billing/metadata headers
      // that change every request (e.g. "x-anthropic-billing-header: ...cch=XXXXX")
      // and break session key stability → preventing resume for Claude Code VSCode
      systemPrompt = req.system
        .filter((block: any) => block?.type === "text" && typeof block?.text === "string")
        .filter((block: any) => !block.text.startsWith("x-anthropic-billing-header:"))
        .map((block: any) => block.text)
        .join("\n")
    } else {
      return { valid: false, error: "system must be a string or array of content blocks" }
    }
  }

  // Parse tools if present.
  // Filter out Anthropic built-in tool types (e.g. web_search_20250305) that have
  // no input_schema — they crash our MCP passthrough. Our proxy provides its own
  // WebSearch via Claude-route tool, so built-in search tools are not needed.
  const rawTools = Array.isArray(req.tools) ? req.tools : undefined
  const tools = rawTools?.filter((t: any) => {
    // Standard tools have { name, input_schema }. Built-in tools have { type: "web_search_20250305", ... }
    if (t && typeof t === 'object' && typeof t.type === 'string' && !t.input_schema) {
      return false  // Skip built-in tool types without input_schema
    }
    return true
  })

  // Parse thinking config (for extended thinking / reasoning)
  let thinking: ThinkingConfig | undefined
  if (req.thinking && typeof req.thinking === "object") {
    const t = req.thinking as Record<string, unknown>
    if (t.type === "enabled" || t.type === "disabled") {
      thinking = {
        type: t.type,
        budget_tokens: typeof t.budget_tokens === "number" ? t.budget_tokens : undefined
      }
    }
  }

  return {
    valid: true,
    data: {
      model: (req.model as string) || "sonnet",
      messages: req.messages as AnthropicMessage[],
      stream: req.stream as boolean | undefined,
      max_tokens: req.max_tokens as number | undefined,
      system: systemPrompt,
      temperature: req.temperature as number | undefined,
      tools: tools as any,
      tool_choice: req.tool_choice as any,
      thinking
    }
  }
}
