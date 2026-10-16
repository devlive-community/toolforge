import type { AppError } from '@toolforge/plugin-ui-sdk'

export const ALGORITHMS = ['md5', 'sha1', 'sha224', 'sha256', 'sha384', 'sha512', 'sha3_256', 'sha3_512', 'sm3', 'crc32'] as const
export type Algorithm = (typeof ALGORITHMS)[number]

export const ALGORITHM_LABELS: Record<Algorithm, string> = {
  md5: 'MD5',
  sha1: 'SHA-1',
  sha224: 'SHA-224',
  sha256: 'SHA-256',
  sha384: 'SHA-384',
  sha512: 'SHA-512',
  sha3_256: 'SHA3-256',
  sha3_512: 'SHA3-512',
  sm3: 'SM3',
  crc32: 'CRC32',
}

export interface TextReport {
  results: { algorithm: Algorithm; digest: string }[]
  bytes: number
  elapsedMs: number
}

export interface FileResult {
  path: string
  name: string
  size: number
  digests: Partial<Record<Algorithm, string>>
  error: AppError | null
}

export interface FilesReport {
  files: FileResult[]
  totalBytes: number
  elapsedMs: number
}

export type KeyEncoding = 'text' | 'hex' | 'base64'
export type OutputEncoding = 'hex' | 'base64'

export interface HmacReport {
  results: { algorithm: Algorithm; mac: string; matches: boolean | null }[]
  keyBytes: number
  elapsedMs: number
}

export const MANIFEST_ALGORITHMS = ['sha256', 'sha512', 'sha1', 'md5', 'sha224', 'sha384', 'sha3_256', 'sha3_512', 'sm3'] as const
export type ManifestAlgorithm = (typeof MANIFEST_ALGORITHMS)[number]
export const MANIFEST_EXTENSIONS: Record<ManifestAlgorithm, string> = {
  md5: 'md5',
  sha1: 'sha1',
  sha224: 'sha224',
  sha256: 'sha256',
  sha384: 'sha384',
  sha512: 'sha512',
  sha3_256: 'sha3-256',
  sha3_512: 'sha3-512',
  sm3: 'sm3',
}

export type ManifestStatus = 'ok' | 'failed' | 'missing' | 'error'

export interface ManifestReport {
  algorithm: Algorithm
  entries: { path: string; status: ManifestStatus; expected: string | null; actual: string | null; error: AppError | null }[]
  truncated: boolean
  summary: { ok: number; failed: number; missing: number; errors: number; invalidLines: number[] }
  totalBytes: number
  elapsedMs: number
  output: string | null
}
