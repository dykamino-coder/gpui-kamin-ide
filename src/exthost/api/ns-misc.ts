// "Less common" namespaces — notebooks, tests, scm, comments, chat,
// lm. Most extensions don't touch these but a few popular ones do
// (Jupyter, GitLens). Stubs let those extensions activate without
// errors; real wiring lands in Phase D.
import { Disposable, noopEvent } from "./shared.js"

export function buildNotebooks() {
  return {
    createNotebookController: () => ({
      dispose() {},
      updateNotebookAffinity() {},
      createNotebookCellExecution: () => ({ start() {}, end() {} }),
    }),
    registerNotebookCellStatusBarItemProvider: () => new Disposable(() => {}),
    onDidChangeNotebookDocument: noopEvent,
  }
}

export function buildTests() {
  return {
    createTestController: () => ({
      dispose() {},
      items: { add() {}, get() { /* no-op */ }, delete() {}, replace() {}, forEach() {}, size: 0 },
      createTestItem: () => ({ children: { add() {}, get() { /* no-op */ }, delete() {} } }),
      createRunProfile: () => ({ dispose() {} }),
      createTestRun: () => ({
        enqueued() {}, started() {}, skipped() {}, failed() {}, passed() {},
        errored() {}, appendOutput() {}, end() {},
      }),
    }),
  }
}

export function buildScm() {
  return {
    createSourceControl: () => ({
      createResourceGroup: () => ({}),
      dispose() {},
    }),
    inputBox: { value: "" },
  }
}

export function buildComments() {
  return {
    createCommentController: () => ({ dispose() {} }),
  }
}

export function buildChat() {
  return {
    createChatParticipant: () => ({ dispose() {} }),
    registerChatVariableResolver: () => new Disposable(() => {}),
  }
}

export function buildLm() {
  return {
    selectChatModels: () => Promise.resolve([] as unknown[]),
    registerTool: () => new Disposable(() => {}),
    tools: [] as unknown[],
    onDidChangeChatModels: noopEvent,
  }
}
