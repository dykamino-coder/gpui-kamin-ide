import { describe, expect, it } from 'vitest'
import { renderSnapshot, wrapCommandWithSnapshot } from './shell-snapshot'

const S = '\u0001OCB-SNAPSHOT\u0001'
const capture = (env: string): string =>
  `profile noise\n${S}functions\nmyfn () \n{ \n    echo hi\n}\n${S}aliases\nalias ll='ls -l'\n${S}shopt\nshopt -s extglob\n` +
  `${S}path\n/home/u/.local/bin\n${S}env\n${env}`

describe('shell snapshot', () => {
  it('replays only what the profile changed, never the capture-time PATH', () => {
    const env = ['PATH', '/usr/bin', 'JAVA_HOME', '/opt/jdk', 'KEEP', 'same', 'PWD', '/x', 'BASH_VERSION', '5'].join('\0') + '\0'
    const out = renderSnapshot(capture(env), { KEEP: 'same', PATH: '/usr/bin' }, 'linux')
    expect(out).toContain('myfn ()')
    expect(out).toContain('shopt -s expand_aliases')
    expect(out).toContain("alias ll='ls -l'")
    expect(out).toContain(`export PATH='/home/u/.local/bin':"$PATH"`)
    expect(out).toContain("export JAVA_HOME='/opt/jdk'")
    expect(out).not.toContain('KEEP')
    expect(out).not.toContain('PWD')
    expect(out).not.toContain('BASH_VERSION')
    expect(out).not.toContain('profile noise')
  })

  it('quotes values with single quotes safely', () => {
    const out = renderSnapshot(capture(`X\0it's\0`), {}, 'linux')
    expect(out).toContain(`export X='it'\\''s'`)
  })

  it('sources the snapshot through a forward-slash path', () => {
    const wrapped = wrapCommandWithSnapshot('echo 1', process.platform === 'win32' ? 'C:\\T\\s.sh' : '/t/s.sh')
    expect(wrapped.split('\n')[0]).toMatch(/^source '(C:\/T\/s\.sh|\/t\/s\.sh)'/)
  })
})
