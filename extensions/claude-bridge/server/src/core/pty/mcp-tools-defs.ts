// MCP tool definitions — the canonical list shipped over `tools/list` to
// the CLI. Extracted from `mcp-http-handler.ts` (Sprint 2 / Stage C, C5)
// because the const itself was 407 LOC and dwarfed the actual handler logic
// in the parent file. Definitions are pure data — no JS deps — so the file
// is safe to import from anywhere without circular issues.

/** Default tool definitions — fallback when session.registeredTools is empty.
 * Expanded to 24+ tools matching the full Electron tool registry. */
export const MCP_TOOLS = [
  // ── File system ─────────────────────────────────────
  {
    name: 'Read',
    description: `Reads a file from the local filesystem. You can access any file directly by using this tool.
Assume this tool is able to read all files on the machine. If the User provides a path to a file assume that path is valid. It is okay to read a file that does not exist; an error will be returned.

Usage:
- The file_path parameter must be an absolute path, not a relative path
- By default, it reads up to 2000 lines starting from the beginning of the file
- You can optionally specify a line offset and limit (especially handy for long files), but it's recommended to read the whole file by not providing these parameters
- Any lines longer than 2000 characters will be truncated
- Results are returned using cat -n format, with line numbers starting at 1
- This tool allows reading images (eg PNG, JPG, etc). When reading an image file the contents are presented visually as a multimodal LLM capability.
- This tool can read PDF files (.pdf). For large PDFs (more than 10 pages), you MUST provide the pages parameter to read specific page ranges (e.g., pages: "1-5"). Reading a large PDF without the pages parameter will fail. Maximum 20 pages per request.
- This tool can read Jupyter notebooks (.ipynb files) and returns all cells with their outputs, combining code, text, and visualizations.
- This tool can only read files, not directories. To read a directory, use an ls command via the Bash tool.
- You will regularly be asked to read screenshots. If the user provides a path to a screenshot, ALWAYS use this tool to view the file at the path. This tool will work with all temporary file paths.
- If you read a file that exists but has empty contents you will receive a system reminder warning in place of file contents.
- OUTPUT SIZE LIMIT: responses are hard-capped at ~720 KB. For big files, pass \`offset\` + \`limit\` and page through — if the reply ends with "[Output truncated by bridge…]" later lines are missing.`,
    inputSchema: {
      type: 'object',
      properties: {
        file_path: { type: 'string', description: 'The absolute path to the file to read' },
        offset: { type: 'number', description: 'The line number to start reading from. Only provide if the file is too large to read at once' },
        limit: { type: 'number', description: 'The number of lines to read. Only provide if the file is too large to read at once.' },
        pages: { type: 'string', description: 'Page range for PDF files (e.g., "1-5", "3", "10-20"). Only applicable to PDF files. Maximum 20 pages per request.' },
      },
      required: ['file_path'],
    },
  },
  {
    name: 'Write',
    description: `Writes a file to the local filesystem.

Usage:
- This tool will overwrite the existing file if there is one at the provided path.
- If this is an existing file, you MUST use the Read tool first to read the file's contents. This tool will fail if you did not read the file first.
- Prefer the Edit tool for modifying existing files — it only sends the diff. Only use this tool to create new files or for complete rewrites.
- NEVER create documentation files (*.md) or README files unless explicitly requested by the User.`,
    inputSchema: {
      type: 'object',
      properties: {
        file_path: { type: 'string', description: 'The absolute path to the file to write (must be absolute, not relative)' },
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
        replace_all: { type: 'boolean', description: 'Replace all occurrences of old_string (default false)', default: false },
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
- OUTPUT SIZE LIMIT: responses are hard-capped at ~720 KB. A root-level \`**/*\` over a big tree will be truncated — pass a tighter \`path\` or a more specific pattern first.`,
    inputSchema: {
      type: 'object',
      properties: {
        pattern: { type: 'string', description: 'The glob pattern to match files against' },
        path: { type: 'string', description: 'The directory to search in. Defaults to current working directory if not specified.' },
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
- Responses are hard-capped at ~720 KB. If you hit the cap, the reply ends with a "[Output truncated by bridge…]" marker and later matches are missing.
- To avoid truncation: narrow \`path\`, add \`glob\`/\`type\`, use \`head_limit\` + \`offset\` for pagination, or switch to \`output_mode: "files_with_matches"\` / \`"count"\` first and drill in afterwards.
- Prefer small \`-C\`/\`-A\`/\`-B\` (<=5) on big repos — every context line multiplies the payload.`,
    inputSchema: {
      type: 'object',
      properties: {
        pattern: { type: 'string', description: 'The regular expression pattern to search for in file contents' },
        path: { type: 'string', description: 'File or directory to search in. Defaults to current working directory.' },
        output_mode: { type: 'string', enum: ['content', 'files_with_matches', 'count'], description: 'Output mode. Defaults to files_with_matches.' },
        glob: { type: 'string', description: 'Glob pattern to filter files (e.g. "*.js", "*.{ts,tsx}")' },
        type: { type: 'string', description: 'File type to search (e.g., js, py, rust). More efficient than glob for standard file types.' },
        multiline: { type: 'boolean', description: 'Enable multiline mode where . matches newlines and patterns can span lines. Default: false.' },
        '-i': { type: 'boolean', description: 'Case insensitive search' },
        '-A': { type: 'number', description: 'Number of lines to show after each match. Requires output_mode: content.' },
        '-B': { type: 'number', description: 'Number of lines to show before each match. Requires output_mode: content.' },
        '-C': { type: 'number', description: 'Alias for context.' },
        context: { type: 'number', description: 'Number of lines to show before and after each match (rg -C). Requires output_mode: content, ignored otherwise.' },
        '-n': { type: 'boolean', description: 'Show line numbers in output. Requires output_mode: content. Defaults to true.' },
        head_limit: { type: 'number', description: 'Limit output to first N lines/entries. Defaults to 0 (unlimited).' },
        offset: { type: 'number', description: 'Skip first N lines/entries before applying head_limit. Defaults to 0.' },
      },
      required: ['pattern'],
    },
  },
  // ── Shell ───────────────────────────────────────────
  {
    name: 'Bash',
    description: `Executes a given bash command and returns its output.

The working directory persists between commands, but shell state does not.

IMPORTANT: Avoid using this tool to run commands when a dedicated tool exists:
- File search: Use Glob (NOT find or ls)
- Content search: Use Grep (NOT grep or rg)
- Read files: Use Read (NOT cat/head/tail)
- Edit files: Use Edit (NOT sed/awk)
- Write files: Use Write (NOT echo/cat)

Instructions:
- Always quote file paths that contain spaces with double quotes
- You may specify an optional timeout in milliseconds (up to 1800000ms / 30 minutes). Default timeout: 1500000ms (25 minutes).
- Use run_in_background to run commands in the background. You will be notified when it finishes.
- Write a clear, concise description of what your command does.

OUTPUT SIZE LIMIT:
- Stdout + stderr combined are hard-capped at ~720 KB. Commands that dump whole logs/dumps will be truncated — pipe through head/tail/grep, or redirect to a file and Read it in chunks.
- If the reply ends with "[Output truncated by bridge…]", later output is missing; re-run with a narrower filter.`,
    inputSchema: {
      type: 'object',
      properties: {
        command: { type: 'string', description: 'The command to execute' },
        timeout: { type: 'number', description: 'Optional timeout in milliseconds (max 1800000 / 30 min)' },
        description: { type: 'string', description: 'Clear, concise description of what this command does' },
        run_in_background: { type: 'boolean', description: 'Set to true to run in the background. Read its output later with BashOutput (by bash_id) and stop it with KillShell — NOT TaskOutput.' },
        dangerouslyDisableSandbox: { type: 'boolean', description: 'Set this to true to dangerously override sandbox mode and run commands without sandboxing.' },
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
- Default timeout: 1500000ms (25 min); max 1800000ms (30 min). Use \`run_in_background\` for longer jobs.
- For file ops use Read/Write/Edit, for content search use Grep, for file search use Glob — not PowerShell equivalents.`,
    inputSchema: {
      type: 'object',
      properties: {
        command: { type: 'string', description: 'The PowerShell command to execute' },
        timeout: { type: 'number', description: 'Optional timeout in milliseconds (max 1800000 / 30 min)' },
        description: { type: 'string', description: 'Clear, concise description of what this command does' },
        run_in_background: { type: 'boolean', description: 'Set to true to run in the background. Read its output later with BashOutput (by bash_id) and stop it with KillShell — NOT TaskOutput.' },
        dangerouslyDisableSandbox: { type: 'boolean', description: 'Override sandbox mode (rare).' },
      },
      required: ['command'],
    },
  },
  {
    name: 'BashOutput',
    description: `Read output from a background shell started with Bash/PowerShell run_in_background:true. Returns the stdout/stderr produced SINCE THE LAST BashOutput call (a per-shell cursor), plus the shell's status (running or exited with code). Call repeatedly to follow a long-running command (dev server, build, watcher). Use the bash_id from the run_in_background result — NOT TaskOutput, which cannot see these client-side background shells.`,
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
    description: `Terminate a background shell started with Bash/PowerShell run_in_background:true, by its bash_id. Sends SIGTERM to the process group (so child workers a dev server spawned are killed too). Use this to stop a server/watcher you started earlier.`,
    inputSchema: {
      type: 'object',
      properties: {
        shell_id: { type: 'string', description: 'The background shell id to terminate (from run_in_background).' },
      },
      required: ['shell_id'],
    },
  },
  // ── Notebook ────────────────────────────────────────
  {
    name: 'NotebookEdit',
    description: 'Edit, insert, or delete a cell in a Jupyter notebook (.ipynb file). Supports code and markdown cells. You must Read the notebook first.',
    inputSchema: {
      type: 'object',
      properties: {
        notebook_path: { type: 'string', description: 'Absolute path to the notebook file' },
        cell_id: { type: 'string', description: 'The ID of the cell to edit (optional — uses cell index if not provided)' },
        new_source: { type: 'string', description: 'New source content for the cell' },
        cell_type: { type: 'string', enum: ['code', 'markdown'], description: 'Type of cell' },
        edit_mode: { type: 'string', enum: ['replace', 'insert', 'delete'], description: 'Edit mode: replace existing cell, insert new cell, or delete cell' },
      },
      required: ['notebook_path', 'new_source'],
    },
  },
  // ── Web ─────────────────────────────────────────────
  {
    name: 'WebFetch',
    description: `Fetches content from a specified URL, converts HTML to markdown, and processes it.

Usage:
- The URL must be a fully-formed valid URL
- HTTP URLs are upgraded to HTTPS unless allow_http is true (e.g. intranet hosts with no TLS)
- The prompt should describe what information you want to extract from the page
- This tool is read-only and does not modify any files
- Results may be summarized if the content is very large
- For GitHub URLs, prefer using the gh CLI via Bash instead (e.g., gh pr view, gh issue view, gh api)`,
    inputSchema: {
      type: 'object',
      properties: {
        url: { type: 'string', description: 'The URL to fetch content from', format: 'uri' },
        prompt: { type: 'string', description: 'A prompt describing what information to extract from the page' },
        allow_http: { type: 'boolean', description: 'Set true to fetch http:// URLs without upgrading to https (for intranet/internal hosts that have no TLS). Default false.' },
      },
      required: ['url', 'prompt'],
    },
  },
  // ── Git ─────────────────────────────────────────────
  {
    name: 'EnterWorktree',
    description: `Creates an isolated git worktree and switches the current session into it. Use this ONLY when the user explicitly asks to work in a worktree.

Requirements: Must be in a git repository. Must not already be in a worktree.
Behavior: Creates a new git worktree with a new branch based on HEAD. Switches the session's working directory to the new worktree.`,
    inputSchema: {
      type: 'object',
      properties: {
        name: { type: 'string', description: 'Optional name for the worktree. If not provided, a random name is generated.' },
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
        path: { type: 'string', description: 'Absolute path of the worktree to remove. Same value EnterWorktree returned in its result.' },
      },
      required: ['path'],
    },
  },
  // ── UI — rendered as widget in Electron ────────────
  {
    name: 'AskUserQuestion',
    description: `Use this tool when you need to ask the user questions during execution. This allows you to:
1. Gather user preferences or requirements
2. Clarify ambiguous instructions
3. Get decisions on implementation choices as you work
4. Offer choices to the user about what direction to take

Usage notes:
- ALWAYS use this tool instead of printing questions as plain text — it provides a much better interactive experience
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
        message: { type: 'string', description: 'Notification body. Keep under ~200 chars — it must fit a small toast.' },
        title: { type: 'string', description: 'Optional notification title. Defaults to "Claude".' },
      },
      required: ['message'],
    },
  },
  {
    name: 'ShowWidget',
    description: `Render an HTML/CSS/SVG widget INLINE in the chat — it appears as a live visual right inside your reply, not as code. Reach for it FREELY and proactively whenever a picture beats prose: charts & graphs (draw with SVG), data tables, diagrams/flowcharts, timelines, dashboards & stat cards, comparison grids, UI mockups, math/geometry, color swatches, progress bars, CSS animations. Don't hold back — if a visual would help the user, show one.

Rules for the HTML:
• Self-contained ONLY. Inline all CSS. External URLs (styles, fonts, images) are BLOCKED — embed images as data: URIs, draw with inline SVG or CSS.
• DECLARATIVE — HTML + CSS + SVG. \`<script>\` does NOT run (rendered in a style-isolated Shadow DOM, not a JS sandbox), so build visuals with SVG/CSS, not JavaScript. CSS animations/transitions DO work.
• Pass an HTML fragment (no <html>/<head> needed). The chat is dark; style for a dark background (light text) — or set your own.
• \`height\` caps the frame (it scrolls if taller); omit for natural height.

This is the way to communicate visually in this chat — prefer it over ASCII art or describing a chart in words.`,
    inputSchema: {
      type: 'object',
      properties: {
        html: { type: 'string', description: 'The self-contained HTML to render inline. Inline all CSS/JS; no external resources.' },
        title: { type: 'string', description: 'Optional caption shown above the widget.' },
        height: { type: 'number', description: 'Optional frame height in px (default 360). The frame scrolls if content is taller.' },
      },
      required: ['html'],
    },
  },
  {
    name: 'TodoList',
    description: 'Return the most recent todo list (as written by TodoWrite) for this session. Use sparingly — on each turn the prior TodoWrite\'s result is already visible in your context, so reading is rarely necessary.',
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
              activeForm: { type: 'string', description: 'Optional present-tense form shown while in_progress (e.g. "Reading files").' },
            },
            required: ['content', 'status'],
          },
        },
      },
      required: ['todos'],
    },
  },

  // ─── LSP — backed by plugin-provided language servers (Electron side) ──
  //
  // These tools are always advertised; when no plugin LSP knows the file's
  // extension they return an error, and Claude falls back to Grep/Read.
  // Positions are zero-based (LSP convention), NOT one-based like our
  // internal file tools.
  {
    name: 'LspDiagnostics',
    description: 'Current diagnostics (errors, warnings) for a file, via a plugin-provided LSP server. File extension determines which LSP is used (e.g. .go → gopls, .py → pyright). Returns a JSON array of LSP Diagnostic objects with range/severity/message.',
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
    description: 'Type/documentation for the symbol at a position, via the LSP server registered for this file extension. Use after Grep to understand an unfamiliar symbol without having to read its entire definition.',
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
    description: 'Location(s) where the symbol under cursor is defined, via LSP. Returns a JSON array of LSP Location objects {uri, range}. Jumps across files.',
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
    description: 'All call sites / references for the symbol under cursor, via LSP. Useful before refactoring — checks impact radius without a slow project-wide Grep.',
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
export const MCP_TOOL_NAMES: readonly string[] = MCP_TOOLS.map(t => t.name)
