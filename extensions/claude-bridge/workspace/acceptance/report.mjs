// Public evidence contains fixed reasons and measurements, never exception text.
export class CheckFailure extends Error {
  constructor(code) { super(code); this.code = code }
}
export function check(value, code) { if (!value) throw new CheckFailure(code) }

export const scenarios = [
  'image-contract', 'credential-storage', 'upstream-health', 'workspace-start', 'browser-login',
  'chat', 'file-tools', 'bash', 'mcp', 'hook', 'file-panel',
  'close-mid-turn', 'reopen-permission', 'two-browser-tabs', 'browser-metrics',
]
// The integrated candidate includes executable/files/snapshots and the memory-only
// credential fix. Failures in those paths now signal regressions. Add only exact
// assertion codes for genuinely pending features, with a dependency and reason.
export const expected = {}
export function outcome(name, error, strict = false, expectedFailures = expected) {
  if (!error) return { name, status: 'PASS' }
  const reason = expectedFailures[name]?.[error.code]
  return { name, status: reason && !strict ? 'XFAIL' : 'FAIL',
    reason: reason ?? `Check ${error instanceof CheckFailure ? error.code : 'UNEXPECTED_ERROR'} failed; raw error omitted.` }
}
export function exitStatus(results) {
  if (results.some(r => r.status === 'FAIL')) return 1
  if (results.some(r => r.status === 'BLOCKED')) return 2
  return 0
}
export function markdown(report) {
  return [
    '# Bridge workspace live acceptance', '',
    `Run: ${report.started}; runner source: ${report.source}${report.sourceDirty ? ' (dirty)' : ''}; image: ${report.imageId ?? 'unavailable'}; image source: ${report.imageSource ?? 'unspecified'}.`, '',
    'Headless Chromium; isolated client HOME/project; no server volume access. No raw logs, transcripts, credentials, screenshots or traces retained.', '',
    '| Scenario | Result | Reason |', '| --- | --- | --- |',
    ...report.results.map(r => `| ${r.name} | ${r.status} | ${r.reason ?? ''} |`),
    '| IDE editor open/reveal/openInVscode | NOT-APPLICABLE | KaminIDE host feature, not Bridge |', '',
    `Measurements: ${JSON.stringify(report.metrics)}.`, '',
    'XFAIL means the named assertion ran and hit a known missing feature. BLOCKED means it did not run. A PASS here requires an executed assertion. Exit 1: failure; exit 2: blocked; exit 0: no failures/blocked (XFAIL remains incomplete acceptance).', '',
  ].join('\n')
}
