import fs from 'node:fs/promises'
import path from 'node:path'
import { createRequire } from 'node:module'

const require = createRequire(new URL('../../../webview/package.json', import.meta.url))
const { rspack } = require('@rspack/core')
const { RspackDevServer: DevServer } = require('@rspack/dev-server')
const { ReactRefreshRspackPlugin: RefreshPlugin } = require('@rspack/plugin-react-refresh')
const { ModuleFederationPlugin } = require('@module-federation/enhanced/rspack')

// A separate real application with the default plugin and dev-server client.
// The package search directory supplies installed test dependencies; there are
// no React aliases, injected runtime imports, or custom HMR accept handlers.
export async function refreshRemote(t, root) {
  const directory = path.join(root, 'demo-mf')
  await fs.mkdir(directory)
  const component = (label, unsafe) => `import {useState} from 'react';
    export default function App() {
      const [count,setCount]=useState(0);
      ${unsafe ? 'const [extra]=useState("new hook");' : ''}
      return <button id="refresh-counter" onClick={()=>setCount(count+1)}>${label} {count}</button>;
    }`
  const source = path.join(directory, 'App.jsx')
  await fs.writeFile(source, component('Before', false))
  await fs.writeFile(path.join(directory, 'index.jsx'), `import App from './App'; import {createRoot} from 'react-dom/client'; createRoot(document.getElementById('root')).render(<App/>);`)
  await fs.writeFile(path.join(directory, 'package.json'), JSON.stringify({ private: true, dependencies: {
    react: '19.3.1', 'react-dom': '19.3.1', '@rspack/core': '2.3.1',
    '@module-federation/enhanced': '2.9.3', '@rspack/plugin-react-refresh': '2.0.2',
  } }))
  const compiler = rspack({ mode: 'development', context: directory, entry: './index.jsx', devtool: false,
    output: { path: path.join(directory, 'dist'), publicPath: 'auto', uniqueName: 'refresh_demo' },
    resolve: { extensions: ['.jsx', '.js', '.json'], modules: [path.dirname(require.resolve('react/package.json')).replace(/[/\\]react$/, '')] },
    module: { rules: [{ test: /\.jsx$/, exclude: /node_modules/, loader: 'builtin:swc-loader', options: {
      jsc: { parser: { syntax: 'ecmascript', jsx: true }, transform: { react: { runtime: 'automatic', development: true, refresh: true } } },
    } }] },
    plugins: [new RefreshPlugin(), new ModuleFederationPlugin({ name: 'refresh_demo', filename: 'remoteEntry.js',
      exposes: { './App': './App.jsx' }, shared: { react: { singleton: true, strictVersion: true, requiredVersion: '19.3.1' }, 'react-dom': { singleton: true, strictVersion: true, requiredVersion: '19.3.1' } },
    })], infrastructureLogging: { level: 'none' }, stats: 'errors-only',
  })
  // preview_mount предписывает publicPath:auto, root entry и полный HMR path.
  // /ws намеренно не обслуживается: старый proxy должен провалить этот тест.
  const server = new DevServer({ host: '127.0.0.1', port: 0,
    webSocketServer: { type: 'ws', options: { path: '/preview/remotes/refresh_demo/ws' } },
  }, compiler)
  const compiled = new Promise((resolve, reject) => compiler.hooks.done.tap('refresh-fixture', stats => stats.hasErrors() ? reject(new Error(stats.toString({ all: false, errors: true }))) : resolve()))
  await server.start()
  t.after(async () => { await server.stop(); await new Promise(resolve => compiler.close(resolve)) })
  await compiled
  return { name: 'refresh_demo', entry: `http://127.0.0.1:${server.server.address().port}/remoteEntry.js`, exposedModule: './App',
    edit: (label, unsafe = false) => fs.writeFile(source, component(label, unsafe)),
  }
}
