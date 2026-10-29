// Модель может меняться после установки промпта; не фиксируем её имя в тексте.
export const RUNTIME_SETTINGS_CONTEXT = `## Claude Code runtime
\`/model\`, \`/status\` and the settings paths the CLI mentions belong to the server container, not the user's machine. The user's local \`~/.claude\` is a different place and local edits reach this session only through Bridge synchronization. Take the active model from \`/status\` or response metadata; if neither is available, say it is unverified.`
