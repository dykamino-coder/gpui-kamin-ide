import { capabilityInventory } from './capability-inventory.js'
import { assertBrowserHostReady, assertCapabilityMatrix, readMatrix } from './capability-matrix.js'

const matrix = readMatrix()
assertCapabilityMatrix(capabilityInventory(), matrix)
assertBrowserHostReady(matrix)
console.log('Capability classifications and declared provider prerequisites pass; see README for remaining full-parity gates.')
