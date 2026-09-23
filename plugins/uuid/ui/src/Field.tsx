import type { ReactNode } from 'react'

export function Field({ label, hint, children }: { label: ReactNode; hint?: ReactNode; children: ReactNode }) {
  return (
    <div className="space-y-1.5">
      <p className="text-xs font-medium text-fg-muted">{label}</p>
      {children}
      {hint && <p className="text-[11px] text-fg-subtle">{hint}</p>}
    </div>
  )
}

export function ToggleRow({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <label className="flex items-center justify-between py-1 text-[13px] text-fg">
      {label}
      {children}
    </label>
  )
}
