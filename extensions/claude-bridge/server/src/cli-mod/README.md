This internal plugin connects one interactive Claude Code terminal session to
the KaminIDE Bridge server through a private Unix socket. The server loads it
by default on Linux; set BRIDGE_CLI_MOD=0 to opt out. It submits plain text as user prompts,
reports main turn lifecycle, and aborts the active turn. It requires the tested
Claude Code version 2.1.312. Before loading the plugin, the server checks the
CLI version. Other versions, failed version probes, non-Linux platforms, and
socket failures use native terminal delivery and OSC status without loading
the plugin. Its handshake and heartbeat also guard against runtime failure.
Native commands, attachments, console input and
transcript persistence remain owned by the interactive CLI. No installation
from a marketplace or account credentials are required by this plugin.
