import { render } from 'preact'
import { App } from './App'
// Fonts and icons ship with the dashboard: corporate networks block or
// redirect public CDNs (cdnjs answered 302 → CORS error, icons missing).
import '@fortawesome/fontawesome-free/css/all.min.css'
import '@fontsource/jetbrains-mono/400.css'
import '@fontsource/jetbrains-mono/500.css'
import '@fontsource/jetbrains-mono/600.css'
import '@fontsource/jetbrains-mono/700.css'
import './assets/styles/variables.css'
import './assets/styles/global.css'

render(<App />, document.getElementById('app')!)
