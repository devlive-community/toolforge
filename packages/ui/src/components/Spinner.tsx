import { LoaderCircle } from 'lucide-react'
import { cn } from '../utils'

export function Spinner({ className }: { className?: string }) {
  return <LoaderCircle aria-hidden className={cn('size-4 animate-spin', className)} />
}
