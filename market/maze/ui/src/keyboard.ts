import type { Side } from './types'
type Key = { key: string; repeat: boolean; altKey: boolean; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean; defaultPrevented: boolean }
export function mazeKey(event: Key, interactive = false): Side | 'consume' | null {
  if (interactive || event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return null
  const side: Side | undefined = ({ ArrowUp: 'top', ArrowRight: 'right', ArrowDown: 'bottom', ArrowLeft: 'left' } as const)[event.key as 'ArrowUp']
  if (!side) return null
  return event.repeat ? 'consume' : side
}
