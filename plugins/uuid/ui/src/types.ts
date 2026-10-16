export type Kind = 'v4' | 'v7' | 'v1' | 'v5' | 'v3' | 'ulid' | 'nanoid' | 'snowflake'
export type Namespace = 'dns' | 'url' | 'oid' | 'x500' | 'custom'

export const KINDS: Kind[] = ['v4', 'v7', 'v1', 'v5', 'v3', 'ulid', 'nanoid', 'snowflake']
export const NAMESPACES: Namespace[] = ['dns', 'url', 'oid', 'x500', 'custom']

export const isUuid = (kind: Kind) => kind.startsWith('v')
export const isNamed = (kind: Kind) => kind === 'v3' || kind === 'v5'

export interface GenerateResult {
  ids: string[]
  deterministic: boolean
  elapsedMs: number
}

export type SnowflakePreset = 'twitter' | 'leaf' | 'baidu' | 'discord' | 'custom'
export const SNOWFLAKE_PRESETS = ['twitter', 'leaf', 'baidu', 'discord'] as const

export interface SnowflakeLayout {
  epochMs: number
  unitMs: number
  datacenterBits: number
  workerBits: number
  sequenceBits: number
}

/** 与 Rust 侧 Preset::layout 一致，仅用于界面展示位数与 ID 上限 */
export const PRESET_LAYOUTS: Record<(typeof SNOWFLAKE_PRESETS)[number], SnowflakeLayout> = {
  twitter: { epochMs: 1288834974657, unitMs: 1, datacenterBits: 5, workerBits: 5, sequenceBits: 12 },
  leaf: { epochMs: 1288834974657, unitMs: 1, datacenterBits: 0, workerBits: 10, sequenceBits: 12 },
  baidu: { epochMs: 1474300800000, unitMs: 1000, datacenterBits: 0, workerBits: 21, sequenceBits: 13 },
  discord: { epochMs: 1420070400000, unitMs: 1, datacenterBits: 5, workerBits: 5, sequenceBits: 12 },
}

export const timeBits = (layout: SnowflakeLayout) => 63 - layout.datacenterBits - layout.workerBits - layout.sequenceBits

export interface SnowflakeOptions {
  preset: SnowflakePreset
  layout: SnowflakeLayout
  datacenterId: number
  workerId: number
}

export interface SnowflakeDecoded {
  preset: Exclude<SnowflakePreset, 'custom'>
  unixMs: number
  timestamp: string | null
  datacenterId: number | null
  workerId: number
  sequence: number
  plausible: boolean
}

export interface InspectResult {
  kind: 'uuid' | 'ulid' | 'snowflake'
  canonical: string
  version: number | null
  variant: string | null
  timestamp: string | null
  unixMs: number | null
  hex: string
  isNil: boolean
  isMax: boolean
  snowflake: SnowflakeDecoded[] | null
}
