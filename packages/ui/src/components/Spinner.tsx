import type { SVGProps } from 'react'
import { LoaderCircle } from 'lucide-react'
import { cn } from '../utils'

export function Spinner({ className, ...props }: Omit<SVGProps<SVGSVGElement>, 'ref'>) {
  return <LoaderCircle aria-hidden {...props} className={cn('size-4 animate-spin', className)} />
}
