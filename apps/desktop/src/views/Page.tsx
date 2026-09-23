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
