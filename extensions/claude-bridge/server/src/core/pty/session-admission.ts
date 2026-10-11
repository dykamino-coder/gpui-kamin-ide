export class SessionAdmissionError extends Error {
  constructor(
    public readonly kind: 'global' | 'token',
    limit: number,
  ) {
    super(
      kind === 'token' ? `Max sessions reached (${limit})` : `Server session limit reached (${limit}); try again later`,
    )
  }
}

// Synchronous reservations cover startup awaits, not just registered PTYs.
// A release is idempotent and conversion to live happens in the same JS turn.
export class SessionAdmission {
  private pending = new Map<string, number>()
  constructor(
    private liveGlobal: () => number,
    private liveToken: (tokenId: string) => number,
    private globalLimit: number,
    private tokenLimit = 10,
  ) {}
  reserve(tokenId: string): () => void {
    const total = [...this.pending.values()].reduce((sum, count) => sum + count, 0)
    if (this.liveGlobal() + total >= this.globalLimit) throw new SessionAdmissionError('global', this.globalLimit)
    const tokenPending = this.pending.get(tokenId) ?? 0
    if (this.liveToken(tokenId) + tokenPending >= this.tokenLimit)
      throw new SessionAdmissionError('token', this.tokenLimit)
    this.pending.set(tokenId, tokenPending + 1)
    let released = false
    return () => {
      if (released) return
      released = true
      const count = (this.pending.get(tokenId) ?? 1) - 1
      if (count) this.pending.set(tokenId, count)
      else this.pending.delete(tokenId)
    }
  }
}
