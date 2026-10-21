/** A requested transcript is absent; no session or PTY was created. */
export class ResumeNotFoundError extends Error {
  constructor() {
    super('Conversation not found')
    this.name = 'ResumeNotFoundError'
  }
}
