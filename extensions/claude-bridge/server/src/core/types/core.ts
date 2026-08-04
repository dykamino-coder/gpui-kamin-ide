// ============================================================================
// Types for Claude Max Proxy
// ============================================================================

export interface Config {
  port: number
  host: string
  debug: boolean
  verbose: boolean
}

export interface AccountInfo {
  email?: string
  organization?: string
  subscriptionType?: string
  apiKeySource?: string
}

export interface Stats {
  requests: number
  errors: number
  webSearches: number
  startedAt: Date
  lastRequestAt: Date | null
  lastResponse: string | null
}

export interface HealthCheck {
  sdk: boolean
  account: AccountInfo | null
  apiPing: number | null
  testResponse?: string
  error?: string
}

// Anthropic API Types
export interface AnthropicMessage {
  role: "user" | "assistant"
  content: string | ContentBlock[]
}

export interface ContentBlock {
  type: "text" | "image" | "tool_use" | "tool_result" | "thinking" | "redacted_thinking"
  text?: string
  source?: {
    type: "base64"
    media_type: string
    data: string
  }
  // tool_use fields
  id?: string
  name?: string
  input?: Record<string, unknown>
  // tool_result fields
  tool_use_id?: string
  content?: string | ContentBlock[]
  is_error?: boolean
  // thinking fields
  thinking?: string
  // redacted_thinking fields
  data?: string
}

export interface ToolDefinition {
  name: string
  description?: string
  input_schema: {
    type: "object"
    properties?: Record<string, unknown>
    required?: string[]
  }
}

export interface ThinkingConfig {
  type: "enabled" | "disabled"
  budget_tokens?: number
}

export interface AnthropicRequest {
  model: string
  messages: AnthropicMessage[]
  max_tokens?: number
  stream?: boolean
  system?: string
  temperature?: number
  tools?: ToolDefinition[]
  tool_choice?: { type: "auto" | "any" | "tool"; name?: string }
  thinking?: ThinkingConfig
}

export interface TokenUsage {
  input_tokens: number
  output_tokens: number
  cache_read_input_tokens?: number
  cache_creation_input_tokens?: number
}

export interface AnthropicResponse {
  id: string
  type: "message"
  role: "assistant"
  content: ContentBlock[]
  model: string
  stop_reason: "end_turn" | "max_tokens" | "stop_sequence" | "tool_use" | null
  usage: {
    input_tokens: number
    output_tokens: number
  }
}

export interface AnthropicStreamEvent {
  type: string
  message?: Partial<AnthropicResponse>
  index?: number
  content_block?: ContentBlock
  delta?: {
    type: "text_delta" | "input_json_delta"
    text?: string
    partial_json?: string
    stop_reason?: string
    stop_sequence?: string | null
  }
  usage?: {
    output_tokens: number
  }
}

export type ClaudeModel = "sonnet" | "opus" | "haiku"
