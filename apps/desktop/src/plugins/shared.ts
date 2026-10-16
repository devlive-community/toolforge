import * as React from 'react'
import * as JsxRuntime from 'react/jsx-runtime'
import * as ReactI18next from 'react-i18next'
import * as UI from '@toolforge/ui'
import * as SDK from '@toolforge/plugin-ui-sdk'

declare global {
  interface Window {
    __TF_SHARED__?: Record<string, unknown>
  }
}

/**
 * 在线安装插件的界面构建时把这些模块改写为从 window.__TF_SHARED__ 取值（见 packages/plugin-build），
 * 保证与宿主共用同一份 React、i18n 实例与组件库。键名必须与 plugin-build 的 SHARED_MODULES 一致。
 */
export function exposeSharedModules() {
  window.__TF_SHARED__ = {
    react: React,
    'react/jsx-runtime': JsxRuntime,
    'react/jsx-dev-runtime': JsxRuntime,
    'react-i18next': ReactI18next,
    '@toolforge/ui': UI,
    '@toolforge/plugin-ui-sdk': SDK,
  }
}
