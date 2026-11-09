// Public evidence contains fixed reasons and measurements, never exception text.
export class CheckFailure extends Error {
  constructor(code) { super(code); this.code = code }
}
export function check(value, code) { if (!value) throw new CheckFailure(code) }

export const scenarios = [
  'image-contract', 'upstream-health', 'workspace-start', 'browser-login',
  'chat', 'file-tools', 'bash', 'mcp', 'hook', 'file-panel',
  'close-mid-turn', 'reopen-permission', 'two-browser-tabs', 'browser-metrics',
]
export const expected = {
  'workspace-start': { CORE_ENTRYPOINT: 'INC-2026-0087: executable composition/environment adapter is not merged.' },
  'file-panel': { FILE_PANEL: '#642: Bridge FilePanel container adapter/reconnect watch is not merged.' },
  'close-mid-turn': {
    SNAPSHOT: 'INC-2026-0087: host snapshot/transcript rehydration and exactly-once reconciliation are not merged.',
  },
  'reopen-permission': { PERMISSION_REHYDRATION: 'INC-2026-0087: pending permission snapshot after closing the last page is not merged.' },
  'two-browser-tabs': { SNAPSHOT: 'INC-2026-0087: targeted host snapshot for simultaneous browser views is not merged.' },
}
export function outcome(name, error, strict = false) {
  if (!error) return { name, status: 'PASS' }
  const reason = expected[name]?.[error.code]
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
