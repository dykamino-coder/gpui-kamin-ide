// Cooperative, independently instrumented child; never reads operator state.
const fs = require('node:fs')
const path = require('node:path')
fs.appendFileSync('monitor-trace.jsonl', JSON.stringify({ pid: process.pid, cwd: process.cwd(),
  plugin: process.env.CLAUDE_PLUGIN_ROOT, data: process.env.CLAUDE_PLUGIN_DATA,
  project: process.env.CLAUDE_PROJECT_DIR, pathFirst: process.env.PATH.split(path.delimiter)[0] }) + '\n')
const control = `monitor-${process.pid}.json`
const timer = setInterval(() => {
  if (!fs.existsSync(control)) return
  const command = JSON.parse(fs.readFileSync(control, 'utf8'))
  fs.unlinkSync(control)
  if (command.stdout) fs.writeSync(1, command.stdout)
  if (command.stderr) fs.writeSync(2, command.stderr)
  if (command.exit !== undefined) { clearInterval(timer); process.exit(command.exit) }
}, 25)
