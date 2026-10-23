import type { Lang } from './catalog'

export const guideTopics = ['start', 'plugins', 'tasks', 'privacy', 'faq', 'agent'] as const
export type GuideTopic = (typeof guideTopics)[number]
export const guideCopy: Record<Lang, Record<GuideTopic, { title: string; heading: string; description: string }>> = {
  zh: {
    start: { title: '快速开始', heading: '从第一个工具开始。', description: '安装 ToolForge，找到需要的工具，完成你的第一个小任务。' },
    plugins: { title: '安装与管理工具', heading: '找到工具，然后带回桌面。', description: '了解内置工具、在线插件、安装与更新的区别。' },
    tasks: { title: '任务与历史记录', heading: '长一点的任务，也心中有数。', description: '查看执行阶段、进度、日志和历史，了解取消与重跑的边界。' },
    privacy: { title: '权限与隐私', heading: '每一项权限，都说清用途。', description: '了解本地处理、网络请求、文件访问和持久化保存的范围。' },
    faq: { title: '常见问题', heading: '几个开始前的小问题。', description: '安装、工具获取、离线使用与版本兼容方面的常见问题。' },
    agent: {
      title: '命令行与 AI Agent',
      heading: 'Agent 也能找到顺手的工具。',
      description: '了解 tfp 命令行、stdio MCP 接入，以及按工具和函数配置的授权方式。',
    },
  },
  en: {
    start: {
      title: 'Quick start',
      heading: 'Start with your first tool.',
      description: 'Install ToolForge, find a useful tool and finish your first small task.',
    },
    plugins: {
      title: 'Install & manage tools',
      heading: 'Find a tool. Bring it to your desktop.',
      description: 'Understand built-in tools, online plugins, installation and updates.',
    },
    tasks: {
      title: 'Tasks & history',
      heading: 'Keep track of longer tasks.',
      description: 'Read stages, progress, logs and history, and understand cancellation and reruns.',
    },
    privacy: {
      title: 'Permissions & privacy',
      heading: 'Understand every permission.',
      description: 'Know the boundaries of local processing, network requests, file access and saved data.',
    },
    faq: {
      title: 'Common questions',
      heading: 'A few things before you begin.',
      description: 'Answers about installation, tools, offline use and compatible versions.',
    },
    agent: {
      title: 'CLI & AI agents',
      heading: 'Useful tools for agents, too.',
      description: 'Connect through tfp and stdio MCP, with tool- and function-level access settings.',
    },
  },
}
