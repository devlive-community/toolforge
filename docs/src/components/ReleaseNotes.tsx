import ReactMarkdown, { type Components } from 'react-markdown'
import remarkGfm from 'remark-gfm'

// GitHub Releases 正文是 markdown：构建时渲染成 HTML（本组件只用于 server 渲染，不进 client bundle）。
// 样式映射与站点 tokens 对齐：小节标题用 font-display，正文 fg-muted，链接 accent。
const components: Components = {
  h1: ({ children }) => <h4 className="mt-4 font-display text-sm font-semibold text-fg">{children}</h4>,
  h2: ({ children }) => <h4 className="mt-4 font-display text-sm font-semibold text-fg">{children}</h4>,
  h3: ({ children }) => <h4 className="mt-4 font-display text-sm font-semibold text-fg">{children}</h4>,
  h4: ({ children }) => <h4 className="mt-4 font-display text-sm font-semibold text-fg">{children}</h4>,
  p: ({ children }) => <p className="my-2 leading-relaxed">{children}</p>,
  ul: ({ children }) => <ul className="my-2 list-disc space-y-1 pl-5 leading-relaxed marker:text-fg-subtle">{children}</ul>,
  ol: ({ children }) => (
    <ol className="my-2 list-decimal space-y-1 pl-5 leading-relaxed marker:text-fg-subtle">{children}</ol>
  ),
  li: ({ children }) => <li className="pl-1">{children}</li>,
  a: ({ href, children }) => (
    <a
      href={href}
      target="_blank"
      rel="noreferrer"
      className="text-accent underline decoration-line underline-offset-2 transition-colors hover:text-accent-strong"
    >
      {children}
    </a>
  ),
  strong: ({ children }) => <strong className="font-semibold text-fg">{children}</strong>,
  code: ({ className, children }) => {
    // fenced code block 的 code 带 language-* class，交给 pre 的样式，不再套徽标
    if (typeof className === 'string' && className.includes('language-')) {
      return <code className="font-mono">{children}</code>
    }
    return (
      <code className="rounded border border-line bg-bg-subtle px-1 py-0.5 font-mono text-[12px] text-fg">
        {children}
      </code>
    )
  },
  pre: ({ children }) => (
    <pre className="my-2 overflow-x-auto rounded-lg border border-line bg-bg-subtle p-3 text-[12px] leading-relaxed text-fg-muted">
      {children}
    </pre>
  ),
  blockquote: ({ children }) => (
    <blockquote className="my-2 border-l-2 border-line-strong pl-3 text-fg-subtle">{children}</blockquote>
  ),
  img: ({ src, alt }) => (
    // eslint-disable-next-line @next/next/no-img-element -- 静态导出不用 next/image 优化管线
    <img src={typeof src === 'string' ? src : undefined} alt={alt ?? ''} loading="lazy" className="my-2 max-w-full rounded-lg border border-line" />
  ),
  hr: () => <hr className="my-3 border-line" />,
  table: ({ children }) => (
    <div className="my-2 overflow-x-auto">
      <table className="w-full border-collapse text-[12px]">{children}</table>
    </div>
  ),
  th: ({ children }) => (
    <th className="border border-line bg-bg-subtle px-2 py-1 text-left font-medium text-fg">{children}</th>
  ),
  td: ({ children }) => <td className="border border-line px-2 py-1">{children}</td>,
}

// 首尾块级元素的边距与容器外边距叠加，用选择器清掉
export function ReleaseNotes({ body }: { body: string }) {
  return (
    <div className="text-[13px] text-fg-muted [&>*:first-child]:mt-0 [&>*:last-child]:mb-0">
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>
        {body}
      </ReactMarkdown>
    </div>
  )
}
