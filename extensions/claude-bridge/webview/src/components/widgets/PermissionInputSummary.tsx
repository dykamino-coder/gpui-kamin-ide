interface PermissionInputSummaryProps {
  input: any
}

export function PermissionInputSummary({ input }: PermissionInputSummaryProps) {
  const inputStr = JSON.stringify(input, null, 2)
  if (inputStr.length <= 2) return null // empty object

  const shortInput = inputStr.length > 200 ? inputStr.substring(0, 200) + '...' : inputStr

  return (
    <div style="font-size:11px;color:var(--text-secondary);background:var(--bg-mantle);padding:6px 8px;border-radius:var(--radius-sm);margin-top:6px;max-height:120px;overflow:auto;white-space:pre-wrap;word-break:break-all;font-family:monospace;">
      {shortInput}
    </div>
  )
}
