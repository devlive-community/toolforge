import { useSyncExternalStore } from 'react'
import { CircleAlert, CircleCheck, Info } from 'lucide-react'
import { cn } from '../utils'

type ToastKind = 'success' | 'error' | 'info'
interface ToastItem {
  id: number
  kind: ToastKind
  message: string
}

let items: ToastItem[] = []
let seq = 0
const listeners = new Set<() => void>()
const emit = () => listeners.forEach((listener) => listener())

function push(kind: ToastKind, message: string, duration = kind === 'error' ? 4000 : 2000) {
  const id = ++seq
  items = [...items.slice(-3), { id, kind, message }]
  emit()
  setTimeout(() => {
    items = items.filter((item) => item.id !== id)
    emit()
  }, duration)
}

export const toast = {
  success: (message: string) => push('success', message),
  error: (message: string) => push('error', message),
  info: (message: string) => push('info', message),
}

const icons = {
  success: <CircleCheck className="text-success" />,
  error: <CircleAlert className="text-danger" />,
  info: <Info className="text-info" />,
}

export function Toaster() {
  const list = useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    () => items,
  )
  return (
    <div className="pointer-events-none fixed inset-x-0 bottom-6 z-toast flex flex-col items-center gap-2" aria-live="polite">
      {list.map((item) => (
        <div
          key={item.id}
          role="status"
          className={cn(
            'flex animate-slide-up items-center gap-2 rounded-popover bg-elevated px-3.5 py-2 text-[13px] text-fg shadow-popover [&_svg]:size-4',
          )}
        >
          {icons[item.kind]}
          {item.message}
        </div>
      ))}
    </div>
  )
}
