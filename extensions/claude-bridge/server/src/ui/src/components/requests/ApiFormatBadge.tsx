import { Badge } from '../shared'

interface ApiFormatBadgeProps {
  endpoint: string
  size?: 'sm' | 'md'
}

export function ApiFormatBadge({ endpoint, size = 'sm' }: ApiFormatBadgeProps) {
  if (endpoint === 'pty') {
    return <Badge variant="info" size={size}>PTY Input</Badge>
  }
  return (
    <Badge variant={endpoint === 'anthropic' ? 'anthropic' : 'openai'} size={size}>
      {endpoint === 'anthropic' ? 'Anthropic' : 'OpenAI'}
    </Badge>
  )
}
