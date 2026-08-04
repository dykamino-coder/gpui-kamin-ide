// Map a file path to a CodeMirror 6 language extension. Tier 1: official
// Lezer-based packages we npm-installed. Tier 2: stream-language modes
// from `@codemirror/legacy-modes` for everything else (yaml-like, sh,
// dockerfile, etc). Tier 3: plain text — no extension, the editor still
// works, just no highlighting.

import type { Extension } from '@codemirror/state'

export async function languageForPath(filePath: string): Promise<Extension | null> {
  const ext = (filePath.match(/\.([A-Za-z0-9]+)$/)?.[1] ?? '').toLowerCase()
  const base = filePath.replace(/^.*[/\\]/, '').toLowerCase()

  // Tier 1 — official Lezer packages, dynamic-imported so unused
  // grammars don't bloat the initial bundle.
  switch (ext) {
    case 'js': case 'jsx': case 'mjs': case 'cjs': {
      const m = await import('@codemirror/lang-javascript')
      return m.javascript({ jsx: ext === 'jsx' })
    }
    case 'ts': case 'tsx': {
      const m = await import('@codemirror/lang-javascript')
      return m.javascript({ jsx: ext === 'tsx', typescript: true })
    }
    case 'json': case 'json5': case 'jsonc': {
      const m = await import('@codemirror/lang-json')
      return m.json()
    }
    case 'md': case 'markdown': case 'mdx': {
      const m = await import('@codemirror/lang-markdown')
      return m.markdown()
    }
    case 'html': case 'htm': case 'xhtml': {
      const m = await import('@codemirror/lang-html')
      return m.html()
    }
    case 'css': case 'scss': case 'sass': case 'less': {
      const m = await import('@codemirror/lang-css')
      return m.css()
    }
    case 'py': case 'pyi': case 'pyx': {
      const m = await import('@codemirror/lang-python')
      return m.python()
    }
    case 'rs': {
      const m = await import('@codemirror/lang-rust')
      return m.rust()
    }
    case 'cpp': case 'cc': case 'cxx': case 'c': case 'h': case 'hpp': case 'hh': case 'hxx': {
      const m = await import('@codemirror/lang-cpp')
      return m.cpp()
    }
    case 'go': {
      const m = await import('@codemirror/lang-go')
      return m.go()
    }
    case 'yaml': case 'yml': {
      const m = await import('@codemirror/lang-yaml')
      return m.yaml()
    }
    case 'sql': case 'mysql': case 'pgsql': {
      const m = await import('@codemirror/lang-sql')
      return m.sql()
    }
    case 'xml': case 'svg': case 'plist': {
      const m = await import('@codemirror/lang-xml')
      return m.xml()
    }
  }

  // Tier 2 — legacy stream-modes for the long tail. Same dynamic-import
  // pattern; StreamLanguage.define() wraps the stream-mode in a real
  // language extension so the rest of the editor (folding, indent,
  // highlight tags) still works.
  const { StreamLanguage } = await import('@codemirror/language')
  switch (ext) {
    case 'sh': case 'bash': case 'zsh': case 'ksh': {
      const { shell } = await import('@codemirror/legacy-modes/mode/shell')
      return StreamLanguage.define(shell)
    }
    case 'toml': case 'cargo': {
      const { toml } = await import('@codemirror/legacy-modes/mode/toml')
      return StreamLanguage.define(toml)
    }
    case 'lua': {
      const { lua } = await import('@codemirror/legacy-modes/mode/lua')
      return StreamLanguage.define(lua)
    }
    case 'rb': {
      const { ruby } = await import('@codemirror/legacy-modes/mode/ruby')
      return StreamLanguage.define(ruby)
    }
    case 'java': {
      const { java } = await import('@codemirror/legacy-modes/mode/clike')
      return StreamLanguage.define(java)
    }
    case 'cs': {
      const { csharp } = await import('@codemirror/legacy-modes/mode/clike')
      return StreamLanguage.define(csharp)
    }
    case 'kt': case 'kts': {
      const { kotlin } = await import('@codemirror/legacy-modes/mode/clike')
      return StreamLanguage.define(kotlin)
    }
    case 'swift': {
      const { swift } = await import('@codemirror/legacy-modes/mode/swift')
      return StreamLanguage.define(swift)
    }
    case 'pl': case 'pm': {
      const { perl } = await import('@codemirror/legacy-modes/mode/perl')
      return StreamLanguage.define(perl)
    }
    case 'hs': case 'lhs': {
      const { haskell } = await import('@codemirror/legacy-modes/mode/haskell')
      return StreamLanguage.define(haskell)
    }
    case 'erl': {
      const { erlang } = await import('@codemirror/legacy-modes/mode/erlang')
      return StreamLanguage.define(erlang)
    }
    case 'ex': case 'exs': case 'elixir': {
      // Elixir not in legacy-modes; nearest is erlang.
      const { erlang } = await import('@codemirror/legacy-modes/mode/erlang')
      return StreamLanguage.define(erlang)
    }
    case 'clj': case 'cljs': case 'cljc': case 'edn': {
      const { clojure } = await import('@codemirror/legacy-modes/mode/clojure')
      return StreamLanguage.define(clojure)
    }
    case 'lisp': case 'cl': case 'el': {
      const { commonLisp } = await import('@codemirror/legacy-modes/mode/commonlisp')
      return StreamLanguage.define(commonLisp)
    }
    case 'diff': case 'patch': {
      const { diff } = await import('@codemirror/legacy-modes/mode/diff')
      return StreamLanguage.define(diff)
    }
    case 'dockerfile': {
      const { dockerFile } = await import('@codemirror/legacy-modes/mode/dockerfile')
      return StreamLanguage.define(dockerFile)
    }
    case 'nginx': {
      const { nginx } = await import('@codemirror/legacy-modes/mode/nginx')
      return StreamLanguage.define(nginx)
    }
    case 'properties': case 'env': case 'gitignore': case 'gitattributes': {
      const { properties } = await import('@codemirror/legacy-modes/mode/properties')
      return StreamLanguage.define(properties)
    }
    case 'tex': case 'latex': case 'sty': case 'cls': {
      const { stex } = await import('@codemirror/legacy-modes/mode/stex')
      return StreamLanguage.define(stex)
    }
    case 'r': case 'rdata': {
      const { r } = await import('@codemirror/legacy-modes/mode/r')
      return StreamLanguage.define(r)
    }
    case 'scala': case 'sbt': {
      const { scala } = await import('@codemirror/legacy-modes/mode/clike')
      return StreamLanguage.define(scala)
    }
  }

  // Filename-based heuristics — pick up Dockerfile, Makefile, etc.
  if (base === 'dockerfile' || base.endsWith('.dockerfile')) {
    const { dockerFile } = await import('@codemirror/legacy-modes/mode/dockerfile')
    return StreamLanguage.define(dockerFile)
  }
  if (base === 'makefile' || base === 'gnumakefile') {
    // Closest fit in legacy-modes — falls through to plain on miss.
  }

  return null
}
