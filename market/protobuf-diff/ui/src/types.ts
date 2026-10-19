// FileDescriptorSet 二进制（base64 或文件路径）解析与兼容性对比的类型定义

export type Severity = 'breaking' | 'conditional' | 'info'

export type Direction = 'oldWriteNewRead' | 'newWriteOldRead' | 'both' | 'protoJson'

export interface Finding {
  severity: Severity
  direction: Direction
  rule: string
  path: string
  oldDesc: string
  newDesc: string
  location: string
}

export interface SetSummary {
  files: number
  messages: number
  enums: number
  services: number
}

export interface UnsupportedFile {
  name: string
  syntax: string
}

export interface Report {
  rulesVersion: string
  oldSummary: SetSummary
  newSummary: SetSummary
  unsupported: UnsupportedFile[]
  findings: Finding[]
  notes: string[]
}

export interface Exported {
  content: string
  ext: string
  markdown: string
}

/** 一侧输入：文件路径或 base64 文本 */
export interface SideInput {
  path?: string
  text?: string
}
