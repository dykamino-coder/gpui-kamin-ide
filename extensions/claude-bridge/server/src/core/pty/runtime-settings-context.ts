// Модель может меняться после установки промпта; не фиксируем её имя в тексте.
// `place` — объявленное хостом место исполнения; без него текст прежний.
export function runtimeSettingsContext(place?: string): string {
  const where = place
    ? `not ${place}. The \`~/.claude\` on ${place} is a different place and edits there reach this session only through Bridge synchronization.`
    : `not the user's machine. The user's local \`~/.claude\` is a different place and local edits reach this session only through Bridge synchronization.`
  return `## Claude Code runtime
\`/model\`, \`/status\` and the settings paths the CLI mentions belong to the server container, ${where} Take the active model from \`/status\` or response metadata; if neither is available, say it is unverified.`
}

export const RUNTIME_SETTINGS_CONTEXT = runtimeSettingsContext()
