export type Locale = 'zh-cn' | 'en-us'
export type Format = 'json' | 'jsonl' | 'csv' | 'sql'
export type Dialect = 'mysql' | 'postgres' | 'sqlite'

export const KINDS = [
  'id', 'uuid', 'name', 'firstName', 'lastName', 'username', 'email', 'phone', 'idCard', 'gender', 'age', 'birthday',
  'date', 'datetime', 'timestamp', 'integer', 'float', 'boolean', 'enum', 'province', 'city', 'address', 'zip',
  'company', 'job', 'url', 'ip', 'ipv6', 'mac', 'color', 'password', 'word', 'sentence', 'paragraph', 'constant',
] as const
export type Kind = (typeof KINDS)[number]

export interface Field {
  /** 仅用于界面列表的稳定 key */
  key: string
  name: string
  kind: Kind
  min?: number
  max?: number
  decimals?: number
  from?: string
  to?: string
  options?: string
  nullPercent: number
}

export interface Config {
  locale: Locale
  rows: number
  seed: number
  format: Format
  dialect: Dialect
  table: string
  fields: Field[]
}

export interface Preview {
  output: string
  seed: number
  rows: number
  previewRows: number
}
