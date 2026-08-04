#!/usr/bin/env npx tsx

// @peculiar/x509 (and tsyringe under it) require this polyfill before any
// `import * as x509` evaluates. Must stay at the top of the entry point.
import "reflect-metadata"
import "../src/index"
