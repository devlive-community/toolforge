export type Sort = 'none' | 'asc' | 'desc' | 'natural' | 'length'

export interface Converted {
  style: string
  output: string
}

export interface LinesOptions {
  trim: boolean
  removeEmpty: boolean
  dedupe: boolean
  ignoreCase: boolean
  sort: Sort
  reverse: boolean
  shuffle: boolean
  number: boolean
  prefix: string
  suffix: string
}

export interface LinesResult {
  output: string
  before: number
  after: number
  duplicates: number
  empty: number
}

export interface Stats {
  characters: number
  charactersNoSpaces: number
  words: number
  cjkCharacters: number
  latinWords: number
  lines: number
  nonEmptyLines: number
  paragraphs: number
  sentences: number
  bytes: number
  readingSeconds: number
  topWords: [string, number][]
}

export const SAMPLE = `ToolForge is a fast, secure and lightweight desktop toolbox.
toolforge keeps your data local and processes everything in Rust.

ToolForge 是一款快速、安全、轻量的桌面工具箱。
所有数据都在本地由 Rust 处理，不会离开你的电脑。
user_profile_id
orderTotalAmount
file10.txt
file2.txt
file2.txt`
