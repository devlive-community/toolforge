/** Rust 侧返回的结构化错误，文案由前端按错误码翻译 */
export interface AppError {
  code: string
  params?: Record<string, unknown>
}

export interface FunctionSpec {
  task?: boolean
}

/** 与 Rust `tf_plugin_api::Manifest` 对应 */
export interface Manifest {
  id: string
  version: string
  name: string
  description: string
  category: string
  keywords: string[]
  accent?: 'violet' | 'blue' | 'green' | 'orange' | 'pink' | 'cyan' | null
  locales: string[]
  permissions: string[]
  functions: Record<string, FunctionSpec>
  sensitive: boolean
}

export function isAppError(value: unknown): value is AppError {
  return typeof value === 'object' && value !== null && typeof (value as AppError).code === 'string'
}

/** 把任意异常规整为 AppError */
export function toAppError(value: unknown): AppError {
  if (isAppError(value)) return value
  return { code: 'app.unknown', params: { detail: String(value) } }
}
