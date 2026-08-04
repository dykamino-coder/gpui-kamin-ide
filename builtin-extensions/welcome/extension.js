const vscode = require("vscode")

function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand("welcome.openWelcome", () => {
      return vscode.window.showInformationMessage("Welcome tab is already open in the editor area.")
    }),
    vscode.commands.registerCommand("welcome.about", () => {
      return vscode.window.showInformationMessage(
        "KaminIDE 0.0.1 — VS Code-compatible extension host. Phase A skeleton.",
      )
    }),
  )
}

function deactivate() {}

module.exports = { activate, deactivate }
