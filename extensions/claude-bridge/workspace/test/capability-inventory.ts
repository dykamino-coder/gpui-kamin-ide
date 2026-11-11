import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import ts from 'typescript'

export interface CapabilitySource { file: string; line: number; shared?: boolean }
export interface CapabilityItem {
  id: string; kind: string; name: string; sources: CapabilitySource[]
  routes?: string[]; handler?: string
}

/** Independent source inventory. Never derives support decisions from registration presence. */
export function capabilityInventory(base = fileURLToPath(new URL('../../', import.meta.url))) {
  const sources = new Map<string, ts.SourceFile>()
  const items = new Map<string, CapabilityItem>()
  const source = (file: string) => {
    if (!sources.has(file)) sources.set(file, ts.createSourceFile(file, fs.readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true))
    return sources.get(file)!
  }
  const walk = (node: ts.Node, visit: (node: ts.Node) => void) => { visit(node); node.forEachChild(child => walk(child, visit)) }
  const name = (node: ts.Node | undefined) => node && (ts.isIdentifier(node) || ts.isStringLiteral(node)) ? node.text : undefined
  const literal = (node: ts.Node | undefined): string | undefined => {
    if (!node) return undefined
    if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) return node.text
    if (ts.isTemplateExpression(node)) return node.head.text + node.templateSpans.map(span => '*' + span.literal.text).join('')
    return undefined
  }
  function add(kind: string, channel: string, file: string, node: ts.Node, shared?: boolean) {
    const id = `${kind}:${channel}`
    if (!items.has(id)) items.set(id, { id, kind, name: channel, sources: [] })
    const item = items.get(id)!
    item.sources.push({ file: path.relative(base, file).replaceAll('\\', '/'),
      line: source(file).getLineAndCharacterOfPosition(node.getStart()).line + 1, ...(shared === undefined ? {} : { shared }) })
    return item
  }
  for (const apiPath of ['webview/shared/types.ts', 'extension/src/shared/types.ts']) {
    const apiFile = path.join(base, apiPath)
    const api = source(apiFile).statements.find(node => ts.isInterfaceDeclaration(node) && node.name.text === 'KaminBridgeApi') as ts.InterfaceDeclaration
    if (!api) throw new Error(`Missing KaminBridgeApi in ${apiPath}`)
    if (api.heritageClauses?.length) throw new Error(`Unclassified inherited API members in ${apiPath}`)
    const names = new Set<string>()
    for (const member of api.members) {
      const memberName = name(member.name)
      if (!memberName) throw new Error('Unclassified API member shape')
      if (names.has(memberName)) throw new Error(`Duplicate API member ${memberName}`)
      names.add(memberName)
      add('api', memberName, apiFile, member)
    }
  }
  const methodRoutes = new Map<string, string[]>()
  function route(method: string, kind: string, channel: string, file: string, node: ts.Node) {
    if (!items.has(`api:${method}`)) add('api', method, file, node)
    const item = add(kind, channel, file, node)
    methodRoutes.set(method, [...(methodRoutes.get(method) ?? []), item.id])
  }
  const tables = path.join(base, 'webview/src/lib/bridge-channels.ts')
  const foundTables = new Set<string>()
  walk(source(tables), node => {
    if (!ts.isVariableDeclaration(node) || !['INVOKE', 'SEND', 'EVENT'].includes(name(node.name) ?? '')) return
    const kind = name(node.name)!.toLowerCase()
    foundTables.add(kind)
    if (!node.initializer || !ts.isObjectLiteralExpression(node.initializer)) throw new Error('Unclassified channel table')
    for (const prop of node.initializer.properties) {
      if (!ts.isPropertyAssignment(prop) || !name(prop.name) || !literal(prop.initializer)) throw new Error('Unclassified channel table member')
      route(name(prop.name)!, kind, literal(prop.initializer)!, tables, prop)
    }
  })
  if (foundTables.size !== 3) throw new Error('Missing channel tables')
  const shim = path.join(base, 'webview/src/bridge-shim.ts')
  const primitives: Record<string, string> = { inv: 'invoke', snd: 'send', sub: 'event' }
  walk(source(shim), node => {
    if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.EqualsToken &&
      ts.isPropertyAccessExpression(node.left) && node.left.expression.getText() === 'bridge') {
      const method = node.left.name.text
      if (!items.has(`api:${method}`)) add('api', method, shim, node)
      walk(node.right, child => {
        if (!ts.isCallExpression(child) || !primitives[child.expression.getText()]) return
        const channel = literal(child.arguments[0]) ?? (method === 'invoke' ? '*' : undefined)
        if (!channel) throw new Error(`Unclassified dynamic shim channel in ${method}`)
        route(method, primitives[child.expression.getText()], channel, shim, child)
      })
    }
    if (ts.isCallExpression(node) && primitives[node.expression.getText()]) {
      const channel = literal(node.arguments[0])
      if (channel) add(primitives[node.expression.getText()], channel, shim, node)
    }
  })
  // Version prefetch serves a synchronous API from this existing invoke.
  methodRoutes.set('getVersion', ['invoke:get-version'])
  for (const [method, routes] of methodRoutes) {
    const item = items.get(`api:${method}`)
    if (!item) throw new Error(`Channel method missing from interface/shim: ${method}`)
    item.routes = [...new Set(routes)].sort()
  }
  function resolve(from: string, specifier: string) {
    const stem = specifier === '@kaminide/host-compat' ? path.join(base, 'extension/src/shim/host-compat')
      : specifier.startsWith('.') ? path.resolve(path.dirname(from), specifier).replace(/\.(js|mjs)$/, '') : undefined
    return stem && [stem, `${stem}.ts`, `${stem}.tsx`, path.join(stem, 'index.ts')].find(file => fs.existsSync(file) && fs.statSync(file).isFile())
  }
  function graph(entry: string) {
    const files = new Set<string>()
    function visit(file: string) {
      if (files.has(file)) return
      files.add(file)
      walk(source(file), node => {
        let specifier: string | undefined
        if ((ts.isImportDeclaration(node) && !node.importClause?.isTypeOnly) || ts.isExportDeclaration(node)) specifier = literal(node.moduleSpecifier)
        if (ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword) specifier = literal(node.arguments[0])
        if (specifier) { const next = resolve(file, specifier); if (next) visit(next) }
      })
    }
    visit(entry)
    return files
  }
  const shared = graph(path.join(base, 'extension/src/bridge-host.ts'))
  const desktop = graph(path.join(base, 'extension/src/index.ts'))
  function files(directory: string): string[] {
    return fs.readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
      const file = path.join(directory, entry.name)
      return entry.isDirectory() ? files(file) : /\.tsx?$/.test(file) && !/\.(test|spec)\./.test(file) ? [file] : []
    })
  }
  const all = new Set([...shared, ...desktop, ...files(path.join(base, 'extension/src')),
    ...files(path.join(base, 'workspace/src')), ...files(path.join(base, 'webview/src')),
    ...files(path.join(base, 'browser-client/src'))])
  function channel(file: string, node: ts.Node | undefined): string | undefined {
    const value = literal(node)
    if (value || !node || !ts.isIdentifier(node)) return value
    let result: string | undefined
    walk(source(file), declaration => {
      if (ts.isVariableDeclaration(declaration) && name(declaration.name) === node.text) result ??= literal(declaration.initializer)
    })
    return result
  }
  function outbound(kind: string, file: string, value: ts.Node | undefined, node: ts.Node) {
    const target = channel(file, value)
    if (target) add(kind, target, file, node, shared.has(file))
    else add('forward', `${path.relative(base, file).replaceAll('\\', '/')}#${value?.getText() ?? '?'}`, file, node, shared.has(file))
  }
  for (const file of all) {
    walk(source(file), node => {
      if (ts.isSwitchStatement(node) && ts.isPropertyAccessExpression(node.expression) && node.expression.name.text === 'channel') {
        for (const clause of node.caseBlock.clauses) {
          if (!ts.isCaseClause(clause)) continue
          const target = channel(file, clause.expression)
          if (!target) throw new Error(`Unclassified channel case: ${file}:${clause.expression.getText()}`)
          add('route', target, file, clause, shared.has(file))
        }
      }
      // Container adapters declare their dispatch sets outside ipcMain.
      if (file.includes(path.join('workspace', 'src')) && ts.isVariableDeclaration(node) && /(?:^|_)(INVOKES|SENDS|EVENTS)$/.test(name(node.name) ?? '')) {
        const kind = name(node.name)!.match(/(?:^|_)(INVOKES|SENDS|EVENTS)$/)![1].slice(0, -1).toLowerCase()
        let count = 0
        if (node.initializer) walk(node.initializer, child => {
          if (ts.isArrayLiteralExpression(child)) for (const value of child.elements) {
            const target = literal(value)
            if (!target) throw new Error(`Unclassified adapter dispatch set: ${file}`)
            add(kind, target, file, value, false)
            count++
          }
        })
        if (!count) throw new Error(`Missing adapter dispatch set: ${file}`)
      }
      if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.EqualsEqualsEqualsToken &&
          [node.left, node.right].some(side => ts.isPropertyAccessExpression(side) && side.name.text === 'channel')) {
        const target = channel(file, ts.isPropertyAccessExpression(node.left) && node.left.name.text === 'channel' ? node.right : node.left)
        if (target) {
          let context: ts.Node | undefined = node.parent
          while (context && !ts.isIfStatement(context)) context = context.parent
          const kind = context && ts.isIfStatement(context) ? context.expression.getText().match(/\.kind\s*===\s*['"](invoke|send|event)['"]/)?.[1] : undefined
          add(kind ?? 'route', target, file, node, shared.has(file))
        } else throw new Error(`Unclassified channel comparison: ${file}:${node.getText()}`)
      }
      if (ts.isCallExpression(node)) {
        const expression = node.expression.getText()
        if (ts.isPropertyAccessExpression(node.expression) &&
          ((node.expression.name.text === 'send' && /(?:\.webContents|\.sender)$/.test(node.expression.expression.getText())) ||
           ['broadcast', 'publish'].includes(node.expression.name.text)) ||
          expression === 'broadcast' || file.replaceAll('\\', '/').endsWith('workspace/src/runtime.ts') && expression === 'send') {
          outbound('event', file, node.arguments[0], node)
        }
        if (['inv', 'snd', 'sub'].includes(expression) && file !== shim) outbound(primitives[expression], file, node.arguments[0], node)
      }
      // Explicit wire frames include private workspace channels absent from the public API.
      if (ts.isObjectLiteralExpression(node)) {
        const prop = (key: string) => node.properties.find(p => 'name' in p && name(p.name) === key)
        const kind = prop('kind'), target = prop('channel')
        if (kind && ts.isPropertyAssignment(kind) && target) {
          const operation = literal(kind.initializer)
          if (['invoke', 'send', 'event'].includes(operation ?? '')) {
            outbound(operation!, file, ts.isPropertyAssignment(target) ? target.initializer : target.name, node)
          }
        }
      }
      if (file.includes(`${path.sep}mcp${path.sep}tools${path.sep}`)) {
        if (ts.isFunctionDeclaration(node) && node.name && node.modifiers?.some(m => m.kind === ts.SyntaxKind.ExportKeyword) &&
            (node.type?.getText() === 'Promise<McpResult>' || node.parameters.some(p => p.type?.getText() === 'Record<string, unknown>'))) {
          add('handler', node.name.text, file, node, shared.has(file))
        }
        if (ts.isVariableStatement(node) && node.modifiers?.some(m => m.kind === ts.SyntaxKind.ExportKeyword)) {
          for (const declaration of node.declarationList.declarations) {
            const fn = declaration.initializer
            if (fn && (ts.isArrowFunction(fn) || ts.isFunctionExpression(fn)) &&
                (fn.type?.getText() === 'Promise<McpResult>' || declaration.type?.getText() === 'ToolHandler' ||
                 fn.parameters.some(p => p.type?.getText() === 'Record<string, unknown>'))) {
              const handler = name(declaration.name)
              if (!handler) throw new Error('Unclassified tool handler declaration')
              add('handler', handler, file, declaration, shared.has(file))
            }
          }
        }
      }
      if (!ts.isCallExpression(node) || !ts.isPropertyAccessExpression(node.expression) || node.expression.expression.getText() !== 'ipcMain') return
      const operation = node.expression.name.text
      if (!['handle', 'handleOnce', 'on', 'once'].includes(operation)) return
      let registered = literal(node.arguments[0])
      if (!registered && ts.isIdentifier(node.arguments[0])) {
        const symbol = node.arguments[0].text
        walk(source(file), declaration => {
          if (ts.isVariableDeclaration(declaration) && name(declaration.name) === symbol) registered = literal(declaration.initializer)
        })
      }
      if (!registered) throw new Error(`Unclassified dynamic IPC registration: ${path.relative(base, file)}:${node.getText().slice(0, 90)}`)
      add(['handle', 'handleOnce'].includes(operation) ? 'invoke' : 'send', registered, file, node, shared.has(file))
    })
  }
  const registry = path.join(base, 'extension/src/main/mcp/tool-registry.ts')
  let foundRegistry = false
  walk(source(registry), node => {
    if (!ts.isVariableDeclaration(node) || name(node.name) !== 'toolRegistry') return
    foundRegistry = true
    if (!node.initializer || !ts.isNewExpression(node.initializer)) throw new Error('Unclassified tool registry shape')
    const array = node.initializer.arguments?.[0]
    if (!array || !ts.isArrayLiteralExpression(array)) throw new Error('Unclassified tool registry entries')
    for (const entry of array.elements) {
      if (!ts.isArrayLiteralExpression(entry) || !literal(entry.elements[0]) || !name(entry.elements[1])) throw new Error('Unclassified tool handler')
      const item = add('tool', literal(entry.elements[0])!, registry, entry)
      if (item.handler) throw new Error(`Duplicate tool ${item.name}`)
      item.handler = name(entry.elements[1])
    }
  })
  if (!foundRegistry) throw new Error('Missing tool registry')
  return [...items.values()].sort((a, b) => a.id.localeCompare(b.id))
}
