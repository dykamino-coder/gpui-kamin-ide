/** The supervising kamin-host increments this on every replacement child. */
export function hostGeneration(): number {
  const value = Number.parseInt(process.env.KAMIN_EXTHOST_GENERATION ?? '0', 10)
  return Number.isSafeInteger(value) && value >= 0 ? value : 0
}
