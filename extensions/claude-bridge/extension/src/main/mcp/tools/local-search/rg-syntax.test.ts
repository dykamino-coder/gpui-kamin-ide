import { describe, expect, it } from 'vitest'
import { compileRgGlob, parseRgGlobLine, rgGlobMatch } from './rg-glob'
import { lineMatcher, translateRustRegex } from './rg-regex'
import { Overrides, parseIgnoreFile } from './rg-ignore'

const globMatches = (glob: string, path: string): boolean => compileRgGlob(glob)!.regex.test(path)

describe('gitignore-style globs', () => {
  it('matches a slashless glob at any depth and anchors one with a slash', () => {
    expect(globMatches('*.ts', 'a/b/c.ts')).toBe(true)
    expect(globMatches('*.ts', 'a/b/c.tsx')).toBe(false)
    expect(globMatches('src/*.ts', 'src/a.ts')).toBe(true)
    expect(globMatches('src/*.ts', 'x/src/a.ts')).toBe(false)
    expect(globMatches('/a', 'a')).toBe(true)
    expect(globMatches('/a', 'b/a')).toBe(false)
  })

  it('gives ** its whole-segment meaning and * no separator', () => {
    expect(globMatches('**/b', 'b')).toBe(true)
    expect(globMatches('a/**/b', 'a/b')).toBe(true)
    expect(globMatches('a/**/b', 'a/x/y/b')).toBe(true)
    expect(globMatches('a/**', 'a/x/y')).toBe(true)
    expect(globMatches('a/**', 'a')).toBe(false)
    expect(globMatches('a*b', 'a/b')).toBe(false)
    expect(globMatches('*.{ts,tsx}', 'x.tsx')).toBe(true)
    expect(globMatches('[!a]*', 'b')).toBe(true)
    expect(globMatches('[!a]*', 'a')).toBe(false)
    expect(compileRgGlob('build/')!.dirOnly).toBe(true)
  })

  it('reads globset syntax: escapes, classes and stars', () => {
    expect(globMatches('a\\*', 'a*')).toBe(true)
    expect(globMatches('a\\*', 'ab')).toBe(false)
    expect(globMatches('\\[x]', '[x]')).toBe(true)
    expect(globMatches('[]a]', ']')).toBe(true)
    expect(globMatches('[a-]', '-')).toBe(true)
    expect(globMatches('[\\]', '\\')).toBe(true)
    expect(globMatches('a**', 'xa/ab')).toBe(true)
    expect(globMatches('a**', 'a/b')).toBe(false)
    expect(globMatches('**/**', 'a/b')).toBe(true)
    expect(globMatches('**', 'a/b')).toBe(true)
    expect(globMatches('x,y', 'x,y')).toBe(true)
  })

  it('reads ignore lines as the ignore crate does', () => {
    const line = (l: string) => parseRgGlobLine(l)
    expect(line('# c')).toBe('skip')
    expect(line('   ')).toBe('skip')
    expect(line('\\#x')).toMatchObject({ bang: false })
    expect(compileRgGlob('\\#x')!.regex.test('#x')).toBe(true)
    expect(compileRgGlob('\\!x')!.regex.test('!x')).toBe(true)
    expect(line('!x')).toMatchObject({ bang: true })
    expect(compileRgGlob('a\\ ')!.regex.test('a ')).toBe(true)
    expect(compileRgGlob('a  ')!.regex.test('a')).toBe(true)
    expect(compileRgGlob('a\\\\b')!.regex.test('a\\b')).toBe(true)
  })

  it('leaves to ripgrep what its versions read differently or reject', () => {
    for (const g of ['{a,{b,c}}', '{a,}', '{}', 'a{**,b}', '[abc', 'a\\', '[b-a]', 'x}', '{a', 'a\\/', '[é]', '']) expect(compileRgGlob(g)).toBeNull()
  })

  it('leaves a byte-wise match on a non-ASCII path to ripgrep', () => {
    expect(rgGlobMatch(compileRgGlob('?.txt')!, 'é.txt')).toBeUndefined()
    expect(rgGlobMatch(compileRgGlob('*.txt')!, 'é.txt')).toBe(true)
  })

  it('applies overrides last-match-first and excludes unmatched files under a whitelist', () => {
    const o = Overrides.compile(['*.ts', '!gen-*'])!
    expect(o.verdict('src/a.ts', false)).toBe('whitelist')
    expect(o.verdict('src/gen-a.ts', false)).toBe('ignore')
    expect(o.verdict('src/a.md', false)).toBe('ignore')
    expect(o.verdict('src', true)).toBeNull()
  })

  it('reads ignore files and refuses lines it does not replicate', () => {
    expect(parseIgnoreFile('# c\n\n*.log\n!keep.log\n')).toHaveLength(2)
    expect(parseIgnoreFile('a \n\\#x\na\r\n')).toHaveLength(3)
    expect(parseIgnoreFile('\uFEFFa\n')).toBeNull()
    expect(parseIgnoreFile('ok\n[abc\n')).toBeNull()
  })
})

describe('Rust regex subset', () => {
  it('keeps the Unicode meaning of the Perl classes and of the dot', () => {
    expect(lineMatcher('\\w+_\\d', false)!.test('слово_٣')).toBe(true)
    expect(lineMatcher('^\\s*$', false)!.test('\u00a0\t')).toBe(true)
    expect(lineMatcher('a.b', false)!.test('a\rb')).toBe(true)
    expect(lineMatcher('\\bcafé\\b', false)!.test('un café.')).toBe(true)
    expect(lineMatcher('\\bcaf\\b', false)!.test('café')).toBe(false)
    expect(lineMatcher('STRASSE|ß', true)!.test('strasse')).toBe(true)
  })

  it('leaves everything else to ripgrep', () => {
    for (const p of ['(?i)a', 'a(?=b)', '\\1', '\\pL', '\\x41', 'a\\n', '[[:alpha:]]', '[a&&b]', '$^', '\\b^', 'a{,2}', 'a**', '(a', 'a)', '\\-', 'a$b']) {
      expect(translateRustRegex(p)).toBeNull()
    }
  })
})
