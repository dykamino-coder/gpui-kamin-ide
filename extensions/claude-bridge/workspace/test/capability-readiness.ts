import { capabilityInventory } from './capability-inventory.js'
import { assertBrowserHostReady, assertCapabilityMatrix, readMatrix } from './capability-matrix.js'

const matrix = readMatrix()
assertCapabilityMatrix(capabilityInventory(), matrix)
assertBrowserHostReady(matrix)
console.log('Every required browser-host capability has an integrated provider.')
