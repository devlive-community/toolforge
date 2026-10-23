import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'

/** 技术文档正文：构建时渲染 Markdown，样式见 .site-markdown */
export function DevMarkdown({ source }: { source: string }) {
  return (
    <div className="site-markdown">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          a: ({ href, children }) =>
            href?.startsWith('http') ? (
              <a href={href} target="_blank" rel="noreferrer">
                {children}
              </a>
            ) : (
              <a href={href}>{children}</a>
            ),
        }}
      >
        {source}
      </ReactMarkdown>
    </div>
  )
}
