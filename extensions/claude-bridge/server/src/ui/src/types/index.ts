export interface Stats {
  requests: number; errors: number; webSearches: number;
  startedAt: string; lastRequestAt: string | null; lastResponse: string | null;
}
export interface ExtendedStats extends Stats {
  userRequests: Record<string, number>;
  userTokens: Record<string, { input: number; output: number }>;
  userModelTokens?: Record<string, Record<string, { input: number; output: number; cacheRead: number; cacheWrite: number }>>;
  lastRequestTimes?: Record<string, string>;
  requestLog: RequestLogEntry[];
}
export interface RequestLogEntry {
  id: string; timestamp: string; endpoint: string; method: string;
  model: string; userName?: string; durationMs: number; inputTokens: number;
  outputTokens: number; cacheReadTokens: number; cacheWriteTokens: number;
  toolsUsed: string[]; status: 'success' | 'error' | 'streaming';
  statusCode: number; error?: string;
  requestBody?: unknown; responseText?: string;
  isUserRequest?: boolean; userMessage?: string;
  sessionKey?: string;
}
export interface UserTimeSeriesEntry { userName: string; period: string; requests: number; inputTokens: number; outputTokens: number }
export interface HealthCheck {
  sessions: number; account: { email?: string; organization?: string; subscriptionType?: string } | null;
  apiPing: number | null; testResponse?: string; error?: string; model?: string;
}
export interface ServerConfig {
  port: number; host: string;
  httpProxy: string | null; httpsProxy: string | null; caCert: string | null;
  logLevel: string;
  maxRequests: number;
}
export interface BridgeEvent { type: string; data: unknown; timestamp: number }
export interface InitData { stats: ExtendedStats; health?: HealthCheck | null }
