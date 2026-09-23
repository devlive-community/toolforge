import type { ComponentType } from 'react'

/** 插件 UI 包的入口模块约定：默认导出工具页面组件 */
export interface PluginModule {
  default: ComponentType
}
