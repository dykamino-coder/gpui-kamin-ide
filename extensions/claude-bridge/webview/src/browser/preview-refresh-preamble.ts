import * as Refresh from 'react-refresh/runtime'

// Install before react-dom/client evaluates and registers its renderer. The
// preview has one runtime, including when several remote builds are loaded.
Refresh.injectIntoGlobalHook(window)
Object.assign(globalThis, {
  $RefreshReg$: () => {},
  $RefreshSig$: Refresh.createSignatureFunctionForTransform,
})
export { Refresh }
