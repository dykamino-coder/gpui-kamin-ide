// Lightweight file-type → FontAwesome icon mapping. Returns class +
// colour so the tree row can style with one inline rule. Keeps the
// palette tied to our accent vars so dark/light themes pick up.

export interface FileIcon {
  cls: string
  color: string
}

export function iconForEntry(name: string, isDir: boolean, expanded: boolean): FileIcon {
  if (isDir) {
    return {
      cls: expanded ? 'fas fa-folder-open' : 'fas fa-folder',
      color: 'var(--accent-primary)',
    }
  }
  const lower = name.toLowerCase()
  const ext = lower.match(/\.([a-z0-9]+)$/)?.[1] ?? ''

  // Filename-based first (ignores extension).
  if (lower === 'package.json' || lower === 'package-lock.json' || lower === 'yarn.lock' || lower === 'pnpm-lock.yaml') {
    return { cls: 'fab fa-npm', color: 'var(--accent-red)' }
  }
  if (lower === 'tsconfig.json' || lower.startsWith('tsconfig.')) {
    return { cls: 'fas fa-cog', color: 'var(--accent-primary)' }
  }
  if (lower === 'dockerfile' || lower.endsWith('.dockerfile') || lower === 'compose.yml' || lower === 'compose.yaml' || lower === 'docker-compose.yml' || lower === 'docker-compose.yaml') {
    return { cls: 'fab fa-docker', color: '#2496ED' }
  }
  if (lower === '.gitignore' || lower === '.gitattributes' || lower === '.gitmodules') {
    return { cls: 'fab fa-git-alt', color: '#f1502f' }
  }
  if (lower === 'readme.md' || lower === 'readme') {
    return { cls: 'fas fa-book', color: 'var(--accent-yellow)' }
  }

  // Extension-based.
  switch (ext) {
    case 'js': case 'mjs': case 'cjs':
      return { cls: 'fab fa-js', color: 'var(--accent-yellow)' }
    case 'ts': case 'tsx':
      return { cls: 'fas fa-file-code', color: '#3178c6' }
    case 'jsx':
      return { cls: 'fab fa-react', color: '#61dafb' }
    case 'json': case 'json5': case 'jsonc':
      return { cls: 'fas fa-code', color: 'var(--accent-yellow)' }
    case 'md': case 'mdx': case 'markdown':
      return { cls: 'fas fa-file-lines', color: 'var(--text-secondary)' }
    case 'html': case 'htm':
      return { cls: 'fab fa-html5', color: '#e34f26' }
    case 'css': case 'scss': case 'sass': case 'less':
      return { cls: 'fab fa-css3-alt', color: '#1572B6' }
    case 'py': case 'pyi':
      return { cls: 'fab fa-python', color: '#3776ab' }
    case 'rs':
      return { cls: 'fab fa-rust', color: 'var(--accent-orange)' }
    case 'go':
      return { cls: 'fas fa-code', color: '#00ADD8' }
    case 'java': case 'jar':
      return { cls: 'fab fa-java', color: '#f89820' }
    case 'rb':
      return { cls: 'fas fa-gem', color: 'var(--accent-red)' }
    case 'php':
      return { cls: 'fab fa-php', color: '#777bb4' }
    case 'sh': case 'bash': case 'zsh':
      return { cls: 'fas fa-terminal', color: 'var(--accent-green)' }
    case 'yaml': case 'yml':
      return { cls: 'fas fa-list', color: 'var(--text-secondary)' }
    case 'toml':
      return { cls: 'fas fa-list', color: 'var(--accent-orange)' }
    case 'sql':
      return { cls: 'fas fa-database', color: 'var(--accent-primary)' }
    case 'env':
      return { cls: 'fas fa-leaf', color: 'var(--accent-green)' }
    case 'png': case 'jpg': case 'jpeg': case 'gif': case 'webp': case 'bmp': case 'ico':
      return { cls: 'fas fa-file-image', color: 'var(--accent-pink)' }
    case 'svg':
      return { cls: 'fas fa-bezier-curve', color: 'var(--accent-purple)' }
    case 'mp4': case 'mov': case 'webm': case 'avi':
      return { cls: 'fas fa-file-video', color: 'var(--accent-purple)' }
    case 'mp3': case 'wav': case 'flac': case 'ogg':
      return { cls: 'fas fa-file-audio', color: 'var(--accent-purple)' }
    case 'zip': case 'tar': case 'gz': case 'rar': case '7z':
      return { cls: 'fas fa-file-zipper', color: 'var(--text-muted)' }
    case 'pdf':
      return { cls: 'fas fa-file-pdf', color: 'var(--accent-red)' }
    case 'lock':
      return { cls: 'fas fa-lock', color: 'var(--text-muted)' }
  }
  return { cls: 'fas fa-file', color: 'var(--text-muted)' }
}
