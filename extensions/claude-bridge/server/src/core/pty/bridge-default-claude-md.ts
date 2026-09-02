/** Shared, tenant-agnostic instructions installed for every Bridge session. */

export const AGENT_TEAMS_REPORTING_CONTRACT = `## Agent Teams delivery contract

If you choose to use Agent Teams for a task:

- Use a finite, named teammate set. Give every teammate a bounded independent task with an explicit deliverable and stop condition.
- Each teammate must send one bounded, self-contained final report through the native \`SendMessage\` tool with \`recipient: "team-lead"\` before becoming idle. Include status, findings, artifact paths, and blockers that affect the result.
- A teammate \`finished\` or \`idle\` notification is lifecycle state, not the report itself. The lead must collect one delivered report from every expected teammate name before synthesizing the result.
- If an expected teammate becomes idle without a delivered report, send at most one recovery \`SendMessage\` to that same teammate asking it to deliver its existing result. Do not create a replacement teammate, retry in a loop, or wait indefinitely.
- If the single recovery request does not produce a report, state that limitation explicitly and synthesize only the reports that were actually delivered.

Agent Teams are optional. Do not create a team merely because the feature is available; use it only when parallel, independent work materially helps the user's task.
`;

/** Tenant-agnostic preamble describing Open Claude Bridge's split architecture
 * (Linux container holds CLI; user's machine holds files + tools). Used both as
 * the per-session CLAUDE.md header and the global ~/.claude/CLAUDE.md default. */
export const BRIDGE_DEFAULT_CLAUDE_MD = `# Open Claude Bridge — Remote Session

You are running inside Open Claude Bridge: a Linux Docker container that hosts
the Claude Code CLI process, while files, shell, and the user's editor live on
their local machine (Windows / macOS / Linux).

## Important

- All file operations (Read, Write, Edit, Glob, Grep) execute on the USER's local machine via MCP tools, not on this server.
- Bash commands also execute on the user's machine.
- The current server-side directory (\`/home/bridge/.claude/bridge-sessions/...\`) is NOT the user's filesystem — do not attempt to access files there directly.
- Always use absolute paths under the user's working directory as the project root.
- When the user asks about "current directory" or "project", they mean their local cwd, not this container.
- Network access from this container goes through the bridge's outbound proxy if configured; the user's local machine has its own network.

## Editor context (auto-attached messages)

When the user has the FilePanel editor open and the **attach file** switch is ON,
every message they send carries an extra fenced block at the end:

    ---
    <!-- editor-context: auto-attached by Open Claude Bridge -->
    The user is currently looking at the file below in the file viewer.
    \`\`\`json editor-context
    {
      "openedFile": "<absolute path>",
      "selectedRange": { "line": N }                    // single-line (caret only)
                      | { "startLine": A, "endLine": B }, // multi-line selection
      "textInSelectedRange": "<selected text>" | null
    }
    \`\`\`

Treat that block as the **implicit subject** of the user's question:
- "this file", "this function", "fix this", "why is it slow" → refers to \`openedFile\`.
- "this line", "the selection", "what does this do" → refers to \`selectedRange\` /
  \`textInSelectedRange\` inside that file.
- The user's natural-language message is the actual instruction; the JSON block
  is purely contextual metadata. Don't echo it back verbatim.
- If the JSON block is absent, the user is asking a general question — do NOT
  assume any particular file is in focus.

${AGENT_TEAMS_REPORTING_CONTRACT}`;
