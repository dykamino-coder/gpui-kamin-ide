// Shared runtime boundary text: never interpolate a model id, which can change
// after this prompt is installed, or credentials from generated settings.
export const RUNTIME_SETTINGS_CONTEXT = `## Claude Code runtime settings

Native /model and /status commands run in the server container, where Claude Code runs. Settings paths mentioned by the CLI refer to that container, not the home directory reached by your MCP tools. The container's ~/.claude/settings.json and the user's local ~/.claude/settings.json are different files.
Bridge restores each conversation's model at launch and supplies --model when it has an explicit selection or transcript evidence; new conversations use the Bridge default. A live /model command changes the running CLI. Do not infer the active model from Windows settings or earlier assistant replies: prefer native /status and the model metadata of an actual response.
Your MCP file tools cannot inspect the container's settings. Do not search the user's computer to disprove a server-side file's existence or claim that a local settings edit will change the server's runtime configuration.`
