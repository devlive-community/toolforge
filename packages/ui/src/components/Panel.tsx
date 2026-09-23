import type { HTMLAttributes, ReactNode } from 'react'
import { cn } from '../utils'

export interface PanelProps extends Omit<HTMLAttributes<HTMLDivElement>, 'title'> {
  title?: ReactNode
  icon?: ReactNode
  extra?: ReactNode
  actions?: ReactNode
  footer?: ReactNode
  bodyClassName?: string
}

/** 工具页面中的面板：标题栏 + 内容 + 状态栏 */
export function Panel({ title, icon, extra, actions, footer, className, bodyClassName, children, ...props }: PanelProps) {
  return (
    <section
      className={cn('flex min-h-0 min-w-0 flex-col overflow-hidden rounded-card border border-border bg-surface shadow-card', className)}
      {...props}
    >
      {(title || actions) && (
        <header className="flex h-12 shrink-0 items-center gap-2 border-b border-border px-3">
          {icon && <span className="flex text-fg-muted [&_svg]:size-4">{icon}</span>}
          {title && <h3 className="truncate text-[13px] font-semibold text-fg">{title}</h3>}
          {extra}
          <div className="ml-auto flex items-center gap-1.5">{actions}</div>
        </header>
      )}
      <div className={cn('min-h-0 flex-1', bodyClassName)}>{children}</div>
      {footer && (
        <footer className="flex h-9 shrink-0 items-center gap-4 border-t border-border bg-surface-2 px-3 text-xs text-fg-muted">
          {footer}
        </footer>
      )}
    </section>
  )
}
