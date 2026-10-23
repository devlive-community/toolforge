import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import type { Lang } from './catalog'

/** 技术文档目录；新增文章时在这里登记，正文放在 src/content/dev/<topic>.<lang>.md */
export const devTopics = ['plugin-development'] as const
export type DevTopic = (typeof devTopics)[number]

export const devCopy: Record<Lang, Record<DevTopic, { title: string; heading: string; description: string }>> = {
  zh: {
    'plugin-development': {
      title: '开发插件',
      heading: '如何开发一个 ToolForge 插件',
      description: '从 manifest、Rust 后端、React 界面到命令行与 AI Agent 接口，完整走一遍在线插件的开发、测试与发布。',
    },
  },
  en: {
    'plugin-development': {
      title: 'Developing plugins',
      heading: 'How to build a ToolForge plugin',
      description: 'Build, test and publish an online plugin: the manifest, the Rust backend, the React interface and the command line and AI agent interface.',
    },
  },
}

/** 构建时读取文章正文（Markdown） */
export function devArticle(topic: DevTopic, lang: Lang): string {
  return readFileSync(join(process.cwd(), 'src', 'content', 'dev', `${topic}.${lang}.md`), 'utf8')
}
