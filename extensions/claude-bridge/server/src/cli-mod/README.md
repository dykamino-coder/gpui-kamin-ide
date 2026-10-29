This internal plugin connects one interactive Claude Code terminal session to
the KaminIDE Bridge server through a private Unix socket. The server loads it
only when BRIDGE_CLI_MOD=1 is enabled. It submits plain text as user prompts,
reports main turn lifecycle, and aborts the active turn. It requires the tested
Claude Code version 2.1.312. Native commands, attachments, console input and
transcript persistence remain owned by the interactive CLI. No installation
from a marketplace or account credentials are required by this plugin.
