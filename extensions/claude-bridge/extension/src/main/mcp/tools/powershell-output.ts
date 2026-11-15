// Owns the buffered stdout of PowerShell commands and its flush at exit.

/**
 * Creates `$__kaminOut`, one buffered writer for stdout and stderr, unless it
 * exists already: a pre-started shell (warm-shell.ts) runs this while it
 * waits, so a command does not pay for it.
 *
 * Errors share the writer, so stdout and stderr keep their order in one
 * stream, as in the native shell tools. Every way out of the process,
 * including [Environment]::Exit() and the host's report of an uncaught error,
 * raises ProcessExit; its handler is a compiled delegate, so it runs without
 * a PowerShell runspace, and flushes what is still buffered.
 */
export const POWERSHELL_OUTPUT_SETUP =
  'if ($null -eq $__kaminOut) { ' +
  '$__kaminOut = [System.IO.TextWriter]::Synchronized((New-Object System.IO.StreamWriter([Console]::OpenStandardOutput(), [Console]::OutputEncoding, 65536))); ' +
  '[Console]::SetOut($__kaminOut); [Console]::SetError($__kaminOut); ' +
  '$__kaminX = [System.Linq.Expressions.Expression]; ' +
  "[AppDomain]::CurrentDomain.add_ProcessExit($__kaminX::Lambda([EventHandler], $__kaminX::Call($__kaminX::Constant($__kaminOut), [System.IO.TextWriter].GetMethod('Flush', [Type[]]@())), " +
  '[System.Linq.Expressions.ParameterExpression[]]@($__kaminX::Parameter([object]), $__kaminX::Parameter([EventArgs]))).Compile()); ' +
  'Remove-Variable __kaminX }; '
