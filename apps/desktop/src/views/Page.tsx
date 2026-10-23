import type { ReactNode } from 'react'

export function Page({ title, description, children }: { title: string; description?: string; children: ReactNode }) {
  return (
    <div className="mx-auto w-full max-w-6xl animate-fade-in px-8 py-8">
      <h1 className="text-2xl font-semibold tracking-tight text-fg">{title}</h1>
      {description && <p className="mt-1.5 text-[13px] text-fg-muted">{description}</p>}
      <div className="mt-6">{children}</div>
    </div>
  )
}

export function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="mb-8">
      <h2 className="mb-3 text-xs font-semibold tracking-wide text-fg-muted uppercase">{title}</h2>
      {children}
    </section>
  )
}

/** 设置项：左侧标题与说明，右侧控件 */
export function Row({ label, hint, warning, children }: { label: string; hint?: string; warning?: string | null; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6 px-5 py-4">
      <div>
        <p className="text-[13px] font-medium text-fg">{label}</p>
        {hint && <p className="mt-0.5 text-xs text-fg-muted">{hint}</p>}
        {warning && <p className="mt-1 text-xs text-warning">{warning}</p>}
      </div>
      {children}
    </div>
  )
}
