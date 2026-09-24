import type { ReactNode } from 'react'
import { cn } from '../utils'

/** 行内片段：由 Rust 侧解析 Markdown 得到，只含纯文本与样式标记 */
export interface MarkdownSpan {
  text: string
  strong?: boolean
  em?: boolean
  strike?: boolean
  code?: boolean
  href?: string
}

export type MarkdownBlock =
  | { type: 'heading'; level: number; spans: MarkdownSpan[] }
  | { type: 'paragraph'; spans: MarkdownSpan[] }
  | { type: 'list'; ordered: boolean; start: number; items: MarkdownBlock[][] }
  | { type: 'quote'; blocks: MarkdownBlock[] }
  | { type: 'code'; text: string }
  | { type: 'rule' }

export interface MarkdownProps {
  blocks: MarkdownBlock[]
  /** 点击链接时调用；不传则链接只作为文本展示 */
  onOpenLink?: (href: string) => void
  className?: string
}

const HEADING = ['text-base', 'text-[15px]', 'text-sm', 'text-[13px]', 'text-[13px]', 'text-[13px]']

function Spans({ spans, onOpenLink }: { spans: MarkdownSpan[]; onOpenLink?: (href: string) => void }) {
  return spans.map((span, index) => {
    let node: ReactNode = span.text
    if (span.code) {
      node = <code className="rounded-sm bg-active px-1 py-px font-mono text-[0.92em]">{node}</code>
    }
    const styled = cn(span.strong && 'font-semibold', span.em && 'italic', span.strike && 'line-through')
    if (span.href && onOpenLink) {
      const href = span.href
      return (
        <a
          key={index}
          href={href}
          onClick={(event) => {
            event.preventDefault()
            onOpenLink(href)
          }}
          className={cn('break-all text-primary underline-offset-2 hover:underline', styled)}
        >
          {node}
        </a>
      )
    }
    return styled ? (
      <span key={index} className={styled}>
        {node}
      </span>
    ) : (
      <span key={index}>{node}</span>
    )
  })
}

function Blocks({ blocks, onOpenLink }: { blocks: MarkdownBlock[]; onOpenLink?: (href: string) => void }) {
  return blocks.map((block, index) => {
    switch (block.type) {
      case 'heading':
        return (
          <p key={index} className={cn('mt-3 font-semibold text-fg first:mt-0', HEADING[block.level - 1])}>
            <Spans spans={block.spans} onOpenLink={onOpenLink} />
          </p>
        )
      case 'paragraph':
        return (
          <p key={index} className="whitespace-pre-wrap">
            <Spans spans={block.spans} onOpenLink={onOpenLink} />
          </p>
        )
      case 'list': {
        const Tag = block.ordered ? 'ol' : 'ul'
        return (
          <Tag
            key={index}
            start={block.ordered ? block.start : undefined}
            className={cn('space-y-1 pl-5', block.ordered ? 'list-decimal' : 'list-disc', 'marker:text-fg-subtle')}
          >
            {block.items.map((item, itemIndex) => (
              <li key={itemIndex} className="space-y-1 pl-0.5">
                <Blocks blocks={item} onOpenLink={onOpenLink} />
              </li>
            ))}
          </Tag>
        )
      }
      case 'quote':
        return (
          <div key={index} className="space-y-2 border-l-2 border-border-strong pl-3 text-fg-muted">
            <Blocks blocks={block.blocks} onOpenLink={onOpenLink} />
          </div>
        )
      case 'code':
        return (
          <pre key={index} className="overflow-x-auto rounded-control bg-active px-3 py-2 font-mono text-xs">
            {block.text}
          </pre>
        )
      case 'rule':
        return <hr key={index} className="border-border" />
    }
  })
}

/** 渲染 Rust 侧解析好的 Markdown 块；不解析、不注入任何 HTML */
export function Markdown({ blocks, onOpenLink, className }: MarkdownProps) {
  return (
    <div className={cn('space-y-2 text-[13px] leading-relaxed text-fg', className)} data-selectable>
      <Blocks blocks={blocks} onOpenLink={onOpenLink} />
    </div>
  )
}
