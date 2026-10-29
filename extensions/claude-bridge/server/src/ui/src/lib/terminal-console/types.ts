/** Public xterm surfaces consumed by both browser clients. Keeping these
 * structural avoids coupling either client's build to the other's dependencies. */
interface Disposable {
  dispose(): void
}
export interface ConsoleInteractionTerminal {
  modes: { mouseTrackingMode: string }
  buffer: {
    active: { type: 'normal' | 'alternate'; baseY: number }
    onBufferChange(listener: () => void): Disposable
  }
  onWriteParsed(listener: () => void): Disposable
  onScroll(listener: () => void): Disposable
  onResize(listener: () => void): Disposable
  attachCustomWheelEventHandler(handler: (event: WheelEvent) => boolean): void
  focus(): void
  hasSelection(): boolean
}
export interface ConsoleClipboardTerminal {
  getSelection(): string
  hasSelection(): boolean
  clearSelection(): void
  selectAll(): void
  paste(text: string): void
  focus(): void
  attachCustomKeyEventHandler(handler: (event: KeyboardEvent) => boolean): void
}
