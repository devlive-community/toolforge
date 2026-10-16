/** 由宿主通过 window.__TF_SHARED__ 提供、插件界面不重复打包的模块 */
export const SHARED_MODULES = [
  'react',
  'react/jsx-runtime',
  'react/jsx-dev-runtime',
  'react-i18next',
  '@toolforge/ui',
  '@toolforge/plugin-ui-sdk',
] as const

export type SharedModule = (typeof SHARED_MODULES)[number]
