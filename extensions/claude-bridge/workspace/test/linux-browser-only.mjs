// Browser processes must never run on the shared Windows desktop, even headless.
if (process.platform !== 'linux') {
  throw new Error('Browser tests require Linux. Run npm run test:browser:container or Linux CI.')
}
