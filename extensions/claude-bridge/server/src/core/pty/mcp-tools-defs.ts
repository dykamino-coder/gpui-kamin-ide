// MCP tool definitions — the canonical list shipped over `tools/list` to
// the CLI. Extracted from `mcp-http-handler.ts` (Sprint 2 / Stage C, C5)
// because the const itself was 407 LOC and dwarfed the actual handler logic
// in the parent file. Definitions are pure data — no JS deps — so the file
// is safe to import from anywhere without circular issues.

/** Default tool definitions — fallback when session.registeredTools is empty.
 * Expanded to 24+ tools matching the full client-host tool registry. */
export const MCP_TOOLS = [
  // ── File system ─────────────────────────────────────
  {
    name: 'Read',
    description: `Reads text files from the user's filesystem, or returns supported images (PNG, JPG, GIF, WebP) as image content. Read screenshots with this tool. PDF rendering is not implemented; extract PDF content with an appropriate host utility first.

Pass an absolute file_path. Text defaults to the first 2000 lines; use offset (1-based) and limit for a smaller range. Lines longer than 2000 characters are truncated. Text is returned with line numbers. Jupyter notebooks are returned as cells with their source and outputs.
Text results are capped at about 720,000 characters; a result containing "[Output truncated by bridge" is incomplete. Narrow the range and read again.`,
    inputSchema: {
      type: 'object',
      properties: {
        file_path: { type: 'string', description: 'The absolute path to the file to read' },
        offset: {
          type: 'number',
          description: 'The line number to start reading from. Only provide if the file is too large to read at once',
        },
        limit: {
          type: 'number',
          description: 'The number of lines to read. Only provide if the file is too large to read at once.',
        },
      },
      required: ['file_path'],
    },
  },
  {
    name: 'Write',
    description: `Writes a file to the local filesystem.

Usage:
- This tool will overwrite the existing file if there is one at the provided path.
- Read an existing file before overwriting it so you can preserve the user's changes.
- Prefer the Edit tool for modifying existing files — it only sends the diff. Only use this tool to create new files or for complete rewrites.
- NEVER create documentation files (*.md) or README files unless explicitly requested by the User.`,
    inputSchema: {
      type: 'object',
      properties: {
        file_path: {
          type: 'string',
          description: 'The absolute path to the file to write (must be absolute, not relative)',
        },
        content: { type: 'string', description: 'The content to write to the file' },
      },
      required: ['file_path', 'content'],
    },
  },
  {
    name: 'Edit',
    description: `Performs exact string replacements in files.

Usage:
- You must use your Read tool at least once in the conversation before editing. This tool will error if you attempt an edit without reading the file.
- The edit will FAIL if old_string is not unique in the file. Either provide a larger string with more surrounding context to make it unique or use replace_all to change every instance of old_string.
- Use replace_all for replacing and renaming strings across the file. This parameter is useful if you want to rename a variable for instance.`,
    inputSchema: {
      type: 'object',
      properties: {
        file_path: { type: 'string', description: 'The absolute path to the file to modify' },
        old_string: { type: 'string', description: 'The text to replace' },
        new_string: { type: 'string', description: 'The text to replace it with (must be different from old_string)' },
        replace_all: {
          type: 'boolean',
          description: 'Replace all occurrences of old_string (default false)',
          default: false,
        },
      },
      required: ['file_path', 'old_string', 'new_string'],
    },
  },
  {
    name: 'Glob',
    description: `Fast file pattern matching tool that works with any codebase size.
- Supports glob patterns like "**/*.js" or "src/**/*.ts"
- Returns matching file paths sorted by modification time
- Use this tool when you need to find files by name patterns
- When you are doing an open ended search that may require multiple rounds of globbing and grepping, use the Agent tool instead
- OUTPUT SIZE LIMIT: responses are hard-capped at about 720,000 characters. A root-level \`**/*\` over a big tree will be truncated — pass a tighter \`path\` or a more specific pattern first.`,
    inputSchema: {
      type: 'object',
      properties: {
        pattern: { type: 'string', description: 'The glob pattern to match files against' },
        path: {
          type: 'string',
          description: 'The directory to search in. Defaults to current working directory if not specified.',
        },
      },
      required: ['pattern'],
    },
  },
  {
    name: 'Grep',
    description: `A powerful search tool built on ripgrep.

Usage:
- ALWAYS use Grep for search tasks. NEVER invoke grep or rg as a Bash command. The Grep tool has been optimized for correct permissions and access.
- Supports full regex syntax (e.g., "log.*Error", "function\\\\s+\\\\w+")
- Filter files with glob parameter (e.g., "*.js", "**/*.tsx") or type parameter (e.g., "js", "py", "rust")
- Output modes: "content" shows matching lines, "files_with_matches" shows only file paths (default), "count" shows match counts
- Use Agent tool for open-ended searches requiring multiple rounds
- Pattern syntax: Uses ripgrep (not grep) - literal braces need escaping
- Multiline matching: By default patterns match within single lines only. For cross-line patterns, use multiline: true

OUTPUT SIZE LIMIT:
- Responses are hard-capped at about 720,000 characters. If you hit the cap, the reply ends with a "[Output truncated by bridge" marker and later matches are missing.
- To avoid truncation: narrow \`path\`, add \`glob\`/\`type\`, use \`head_limit\` + \`offset\` for pagination, or switch to \`output_mode: "files_with_matches"\` / \`"count"\` first and drill in afterwards.
- Prefer small \`-C\`/\`-A\`/\`-B\` (<=5) on big repos — every context line multiplies the payload.`,
    inputSchema: {
      type: 'object',
      properties: {
        pattern: { type: 'string', description: 'The regular expression pattern to search for in file contents' },
        path: { type: 'string', description: 'File or directory to search in. Defaults to current working directory.' },
        output_mode: {
          type: 'string',
          enum: ['content', 'files_with_matches', 'count'],
          description: 'Output mode. Defaults to files_with_matches.',
        },
        glob: { type: 'string', description: 'Glob pattern to filter files (e.g. "*.js", "*.{ts,tsx}")' },
        type: {
          type: 'string',
          description: 'File type to search (e.g., js, py, rust). More efficient than glob for standard file types.',
        },
        multiline: {
          type: 'boolean',
          description: 'Enable multiline mode where . matches newlines and patterns can span lines. Default: false.',
        },
        '-i': { type: 'boolean', description: 'Case insensitive search' },
        '-A': {
          type: 'number',
          description: 'Number of lines to show after each match. Requires output_mode: content.',
        },
        '-B': {
          type: 'number',
          description: 'Number of lines to show before each match. Requires output_mode: content.',
        },
        '-C': { type: 'number', description: 'Alias for context.' },
        context: {
          type: 'number',
          description:
            'Number of lines to show before and after each match (rg -C). Requires output_mode: content, ignored otherwise.',
        },
        '-n': {
          type: 'boolean',
          description: 'Show line numbers in output. Requires output_mode: content. Defaults to true.',
        },
        head_limit: {
          type: 'number',
          description: 'Limit output to first N lines/entries. Defaults to 0 (unlimited).',
        },
        offset: {
          type: 'number',
          description: 'Skip first N lines/entries before applying head_limit. Defaults to 0.',
        },
      },
      required: ['pattern'],
    },
  },
  // ── Shell ───────────────────────────────────────────
  {
    name: 'Bash',
    description: `Executes a given bash command and returns its output.

Each command starts in the session's working directory. Changes made with cd inside a command do not persist to the next call. Shell state does not persist.

IMPORTANT: Avoid using this tool to run commands when a dedicated tool exists:
- File search: Use Glob (NOT find or ls)
- Content search: Use Grep (NOT grep or rg)
- Read files: Use Read (NOT cat/head/tail)
- Edit files: Use Edit (NOT sed/awk)
- Write files: Use Write (NOT echo/cat)

Instructions:
- Always quote file paths that contain spaces with double quotes
- You may specify an optional timeout in milliseconds (up to 1800000ms / 30 minutes). Default timeout: 1500000ms (25 minutes).
- With MCP Tasks support, foreground commands finish inline within two minutes or return a durable MCP task. Use run_in_background for background commands. With MCP Tasks support, the task returns immediately and the CLI handles completion notifications and tasks/get results. Without Tasks support, poll BashOutput with bash_id; there is no automatic completion notification.
- Write a clear, concise description of what your command does.

OUTPUT SIZE LIMIT:
- Output is bounded to 100 KB per stream, including MCP task results. Commands that dump whole logs/dumps will be truncated — filter the output, or redirect to a file and Read it in chunks.
- If the reply contains an output truncation notice, re-run with a narrower filter.`,
    inputSchema: {
      type: 'object',
      properties: {
        command: { type: 'string', description: 'The command to execute' },
        timeout: { type: 'number', description: 'Optional timeout in milliseconds (max 1800000 / 30 min)' },
        description: { type: 'string', description: 'Clear, concise description of what this command does' },
        run_in_background: {
          type: 'boolean',
          description:
            'Set to true to run in the background. With MCP Tasks support, returns a task immediately; completion notifies the agent, output is available through tasks/get, cancellation through tasks/cancel (up to 24 hours). Otherwise use BashOutput (bash_id) and KillShell.',
        },
      },
      required: ['command'],
    },
  },
  {
    name: 'PowerShell',
    description: `Executes a PowerShell 7+ command on the user's Windows host and returns its output. Use this for Windows-specific operations the Bash tool can't handle: registry access (\`HKLM:\\\`/\`HKCU:\\\`), \`Get-Process\`/\`Stop-Process\`, IShellLink/COM via \`shell.writeShortcutLink\`, native Win32 APIs, scheduled tasks, services, etc.

Notes:
- pwsh 7+ syntax: \`&&\`/\`||\` chain operators ARE available, ternary (\`$cond ? $a : $b\`), null-coalescing (\`??\`).
- Variables: \`$x = "value"\`. Escape char is backtick \`\`\`.
- Pipe \`|\` passes objects, not text — use \`Select-Object\`/\`Where-Object\`/\`ForEach-Object\` for filtering.
- Call native exe with spaces in path via \`& "C:\\Program Files\\App\\app.exe" arg1 arg2\`.
- Multiline strings: single-quoted here-string \`@'…'@\` is literal (no expansion); the closing \`'@\` MUST be at column 0.
- Stop-parsing token \`--%\` lets you pass args literally to native exes.

Restrictions:
- Runs with \`-NonInteractive\`. Never use \`Read-Host\`, \`Get-Credential\`, \`pause\`, or interactive editors.
- Add \`-Confirm:$false\` for destructive cmdlets that prompt.
- Each call starts a new process in the session working directory; cd and environment changes do not persist.
- Foreground timeout: 1500000ms (25 min) by default, max 1800000ms (30 min). With MCP Tasks support, output returns inline within two minutes or as a durable task. Explicit background tasks return immediately and can run for almost 24 hours.
- For file ops use Read/Write/Edit, for content search use Grep, for file search use Glob — not PowerShell equivalents.`,
    inputSchema: {
      type: 'object',
      properties: {
        command: { type: 'string', description: 'The PowerShell command to execute' },
        timeout: { type: 'number', description: 'Optional timeout in milliseconds (max 1800000 / 30 min)' },
        description: { type: 'string', description: 'Clear, concise description of what this command does' },
        run_in_background: {
          type: 'boolean',
          description:
            'Set to true to run in the background. With MCP Tasks support, returns a task immediately; completion notifies the agent, output is available through tasks/get, cancellation through tasks/cancel (up to 24 hours). Otherwise use BashOutput (bash_id) and KillShell.',
        },
      },
      required: ['command'],
    },
  },
  {
    name: 'BashOutput',
    description: `Read output from a legacy background shell started without MCP Tasks support by Bash/PowerShell run_in_background:true. Returns the stdout/stderr produced SINCE THE LAST BashOutput call (a per-shell cursor), plus the shell's status (running or exited with code). Call repeatedly to follow a long-running command (dev server, build, watcher). Use the bash_id from the run_in_background result — NOT TaskOutput, which cannot see these client-side background shells.`,
    inputSchema: {
      type: 'object',
      properties: {
        bash_id: { type: 'string', description: 'The background shell id returned by run_in_background.' },
        filter: { type: 'string', description: 'Optional regex — only stdout lines matching it are returned.' },
      },
      required: ['bash_id'],
    },
  },
  {
    name: 'KillShell',
    description: `Terminate a legacy background shell started without MCP Tasks support by Bash/PowerShell run_in_background:true, by its bash_id. On POSIX, sends SIGTERM to the process group; on Windows, terminates the shell process. Use this to stop a server/watcher you started earlier.`,
    inputSchema: {
      type: 'object',
      properties: {
        bash_id: { type: 'string', description: 'The bash_id returned by run_in_background, for Bash or PowerShell.' },
      },
      required: ['bash_id'],
    },
  },
  // ── Notebook ────────────────────────────────────────
  {
    name: 'NotebookEdit',
    description:
      'Edit, insert, or delete a cell in a Jupyter notebook (.ipynb file). Supports code and markdown cells. You must Read the notebook first.',
    inputSchema: {
      type: 'object',
      properties: {
        notebook_path: { type: 'string', description: 'Absolute path to the notebook file' },
        cell_id: {
          type: 'string',
          description: 'The ID of the cell to edit (optional — uses cell index if not provided)',
        },
        new_source: { type: 'string', description: 'New source content for the cell' },
        cell_type: { type: 'string', enum: ['code', 'markdown'], description: 'Type of cell' },
        edit_mode: {
          type: 'string',
          enum: ['replace', 'insert', 'delete'],
          description: 'Edit mode: replace existing cell, insert new cell, or delete cell',
        },
      },
      required: ['notebook_path', 'new_source'],
    },
  },
  // ── Web ─────────────────────────────────────────────
  {
    name: 'WebFetch',
    description: `Fetches public URL content on the user's machine and returns it, converting HTML to markdown. No separate model processes the prompt: inspect the returned content yourself. Large content is truncated, not summarized.

HTTP is upgraded to HTTPS unless allow_http is true. Localhost and private/internal addresses are blocked, including redirect targets; allow_http does not bypass this protection. This tool is read-only.`,
    inputSchema: {
      type: 'object',
      properties: {
        url: { type: 'string', description: 'The URL to fetch content from', format: 'uri' },
        prompt: {
          type: 'string',
          description: 'Context for the fetch request; returned with the content, without model processing',
        },
        allow_http: {
          type: 'boolean',
          description:
            'Set true to fetch public http:// URLs without upgrading to HTTPS. Private/internal hosts remain blocked. Default false.',
        },
      },
      required: ['url', 'prompt'],
    },
  },
  // ── Git ─────────────────────────────────────────────
  {
    name: 'EnterWorktree',
    description: `Creates a git worktree under the host repository's .claude/worktrees directory, with a worktree/<name> branch based on HEAD. Returns the path and branch; does not switch the session working directory. Use the returned path explicitly in subsequent operations. Use ONLY when the user explicitly asks for a worktree. Requires the host process working directory to be inside a git repository.`,
    inputSchema: {
      type: 'object',
      properties: {
        name: {
          type: 'string',
          description: 'Optional name for the worktree. If not provided, a random name is generated.',
        },
      },
    },
  },
  {
    name: 'ExitWorktree',
    description: `Removes a git worktree previously created with EnterWorktree. The worktree must exist on disk; the matching branch is left intact unless the caller deletes it separately.

Use only after the user is finished with the worktree. Use \`git worktree remove --force\` semantics — uncommitted changes inside the worktree are discarded.`,
    inputSchema: {
      type: 'object',
      properties: {
        path: {
          type: 'string',
          description: 'Absolute path of the worktree to remove. Same value EnterWorktree returned in its result.',
        },
      },
      required: ['path'],
    },
  },
  // ── UI — rendered as a widget in KaminIDE ──────────
  {
    name: 'AskUserQuestion',
    description: `Use this tool when you need to ask the user questions during execution. This allows you to:
1. Gather user preferences or requirements
2. Clarify ambiguous instructions
3. Get decisions on implementation choices as you work
4. Offer choices to the user about what direction to take

Usage notes:
- Use this tool when an answer is needed; if unavailable or failing, ask briefly in text. Failure is not an answer or approval.
- Users will always be able to type a custom answer
- If you recommend a specific option, make that the first option in the list and add "(Recommended)" at the end
- The user sees a styled widget with clickable buttons`,
    inputSchema: {
      type: 'object',
      properties: {
        questions: {
          type: 'array',
          description: 'Array of questions to ask the user',
          items: {
            type: 'object',
            properties: {
              question: { type: 'string', description: 'The question text to display' },
              header: { type: 'string', description: 'Optional header/title for the question' },
              options: {
                type: 'array',
                description: 'Answer choices displayed as clickable buttons',
                items: {
                  type: 'object',
                  properties: {
                    label: { type: 'string', description: 'Option text shown on button' },
                    description: { type: 'string', description: 'Additional description for the option' },
                    preview: { type: 'string', description: 'Preview text for the option' },
                  },
                  required: ['label'],
                },
              },
              multiSelect: { type: 'boolean', description: 'Allow selecting multiple options (default: false)' },
            },
            required: ['question'],
          },
        },
      },
      required: ['questions'],
    },
  },
  // NOTE: Agent, SendMessage, TaskCreate/Get/Update/List/Output/Stop,
  // EnterPlanMode, ExitPlanMode, Skill, ToolSearch, WebSearch
  // are ALL native CLI tools — NOT provided via MCP. (TeamCreate/TeamDelete
  // were removed from the CLI in 2.1.178 — implicit single team per session.)

  // ── Notification & todo (bridge-side) ───────────────
  {
    name: 'PushNotification',
    description: `Send a desktop notification to the user via the bridge's toast pipeline. When the main window is focused the toast appears in-app; when it isn't, an OS-level toast window pops in the bottom-right corner. Use sparingly — for finished work, important results, or time-sensitive prompts. Mobile push (Remote Control) is not wired in this client; this is desktop-only.`,
    inputSchema: {
      type: 'object',
      properties: {
        message: {
          type: 'string',
          description: 'Notification body. Keep under ~200 chars — it must fit a small toast.',
        },
        title: { type: 'string', description: 'Optional notification title. Defaults to "Claude".' },
      },
      required: ['message'],
    },
  },
  {
    name: 'ShowWidget',
    description: `Render an HTML/CSS/SVG widget inline in the chat when a picture is clearly easier to understand than text: charts, diagrams or visual comparisons. Do not duplicate a table that Markdown can express clearly. Keep HTML/SVG concise: it consumes output tokens.

Pass a self-contained HTML fragment with inline CSS. External resources are blocked; embed images as data: URIs or draw with SVG/CSS. Script elements do not execute in the style-isolated Shadow DOM; use declarative HTML/CSS/SVG. CSS animations and transitions work.
Theme CSS variables inherit into the Shadow DOM: use var(--text-primary), var(--text-secondary), var(--bg-surface), var(--bg-overlay), var(--accent-primary) and var(--accent-action). Reserve literal colors for contrasting data marks. The host text and links follow the active theme.
The height parameter caps the frame with scrolling; omit it for natural height.`,
    inputSchema: {
      type: 'object',
      properties: {
        html: {
          type: 'string',
          description:
            'The self-contained HTML to render inline. Inline all CSS; use declarative HTML/SVG and no external resources.',
        },
        title: { type: 'string', description: 'Optional caption shown above the widget.' },
        height: {
          type: 'number',
          description:
            'Optional cap on the widget height in px — it scrolls if the content is taller. Omit for natural height.',
        },
      },
      required: ['html'],
    },
  },
  {
    name: 'TodoList',
    description:
      "Return the most recent todo list (as written by TodoWrite) for this session. Use sparingly — on each turn the prior TodoWrite's result is already visible in your context, so reading is rarely necessary.",
    inputSchema: { type: 'object', properties: {} },
  },
  {
    name: 'TodoWrite',
    description: `Track progress on a multi-step task by writing the full todo list. Replaces any prior list for this session. Each call should send the full updated set of todos — partial updates are not merged. Use whenever a task has 3+ steps to give the user visibility into progress.`,
    inputSchema: {
      type: 'object',
      properties: {
        todos: {
          type: 'array',
          description: 'Full ordered list of todos for this task.',
          items: {
            type: 'object',
            properties: {
              content: { type: 'string', description: 'Imperative description of the step.' },
              status: { type: 'string', enum: ['pending', 'in_progress', 'completed'] },
              activeForm: {
                type: 'string',
                description: 'Optional present-tense form shown while in_progress (e.g. "Reading files").',
              },
            },
            required: ['content', 'status'],
          },
        },
      },
      required: ['todos'],
    },
  },

  // ─── LSP — backed by plugin-provided language servers (client side) ────
  //
  // These tools are always advertised; when no plugin LSP knows the file's
  // extension they return an error, and Claude falls back to Grep/Read.
  // Positions are zero-based (LSP convention), NOT one-based like our
  // internal file tools.
  {
    name: 'LspDiagnostics',
    description:
      'Current diagnostics (errors, warnings) for a file, via a plugin-provided LSP server. File extension determines which LSP is used (e.g. .go → gopls, .py → pyright). Returns a JSON array of LSP Diagnostic objects with range/severity/message.',
    inputSchema: {
      type: 'object',
      properties: {
        file: { type: 'string', description: 'Absolute path to a file. Relative paths are rejected.' },
      },
      required: ['file'],
    },
  },
  {
    name: 'LspHover',
    description:
      'Type/documentation for the symbol at a position, via the LSP server registered for this file extension. Use after Grep to understand an unfamiliar symbol without having to read its entire definition.',
    inputSchema: {
      type: 'object',
      properties: {
        file: { type: 'string', description: 'Absolute path.' },
        line: { type: 'number', description: 'Zero-based line number.' },
        character: { type: 'number', description: 'Zero-based column. Defaults to 0.' },
      },
      required: ['file', 'line'],
    },
  },
  {
    name: 'LspDefinition',
    description:
      'Location(s) where the symbol under cursor is defined, via LSP. Returns a JSON array of LSP Location objects {uri, range}. Jumps across files.',
    inputSchema: {
      type: 'object',
      properties: {
        file: { type: 'string', description: 'Absolute path.' },
        line: { type: 'number', description: 'Zero-based line.' },
        character: { type: 'number', description: 'Zero-based column. Defaults to 0.' },
      },
      required: ['file', 'line'],
    },
  },
  {
    name: 'LspReferences',
    description:
      'All call sites / references for the symbol under cursor, via LSP. Useful before refactoring — checks impact radius without a slow project-wide Grep.',
    inputSchema: {
      type: 'object',
      properties: {
        file: { type: 'string', description: 'Absolute path.' },
        line: { type: 'number', description: 'Zero-based line.' },
        character: { type: 'number', description: 'Zero-based column. Defaults to 0.' },
      },
      required: ['file', 'line'],
    },
  },
]

/** Names of every tool we advertise via tools/list. Derived from MCP_TOOLS so
 *  there's only one source of truth — when you add a tool to MCP_TOOLS it
 *  automatically appears here, and `session-core.ts:DISALLOWED_BUILTIN_TOOLS`
 *  picks it up to deny the matching CLI native (so the model uses our MCP
 *  version instead of the CLI's own implementation). */
export const MCP_TOOL_NAMES: readonly string[] = MCP_TOOLS.map((t) => t.name)
