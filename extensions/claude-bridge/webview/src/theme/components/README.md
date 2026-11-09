# Current webview root styles

The chat, tools, customize and browser workspace entries import these shared
styles in order after global/theme tokens:

1. `chat.css`: root surfaces, overlays, agents, chat and terminal containers.
2. `jsonl.css`: transcript entries, content blocks and viewer states.
3. `widgets.css`: input, permissions, tools/customize panels and shared controls.

These are the styles used by current KaminIDE roots. The standalone client,
window chrome and sidebar styles were removed. Keep colours, spacing, radii
and fonts tied to the shared `var(--*)` theme tokens. Component-specific new
styles should live next to their component; these files preserve the existing
cascade for the shared roots.
