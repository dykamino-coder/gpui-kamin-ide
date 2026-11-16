import { describe, expect, it } from 'vitest'
import { compileRgGlob } from './rg-glob'
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

  it('leaves syntax outside the replicated subset to ripgrep', () => {
    for (const g of ['a**', '**/**', 'a\\*', '[]a]', '{a,{b,c}}', '{a/b,c}', '[[:alpha:]]', '']) expect(compileRgGlob(g)).toBeNull()
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
    expect(parseIgnoreFile('a \n')).toBeNull()
    expect(parseIgnoreFile('\\#x\n')).toBeNull()
    expect(parseIgnoreFile('a\r\n')).toBeNull()
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
