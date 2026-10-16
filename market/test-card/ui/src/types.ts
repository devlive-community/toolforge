export const BRANDS = ['visa', 'mastercard', 'amex', 'discover', 'jcb', 'unionpay', 'diners'] as const
export type Brand = (typeof BRANDS)[number]

export interface Card {
  number: string
  brand: Brand
  holder: string
  expiry: string
  cvc: string
}

export interface Generated {
  cards: Card[]
  seeded: boolean
  algorithm: string
  csv: string
  json: string
}

export type Category = 'success' | 'decline' | 'authentication' | 'fraud' | 'dispute'

export interface SandboxCase {
  platform: string
  number: string
  brand: Brand
  category: Category
  scenario: string
  code: string | null
  source: string
  verified: string
}

export type Problem = 'characters' | 'unknownBrand' | 'length' | 'luhn'

export interface Checked {
  line: number
  input: string
  digits: string
  brand: Brand | null
  length: number
  allowedLengths: number[]
  luhn: boolean
  problems: Problem[]
  sandbox: SandboxCase | null
}

/** 每 4 位分组显示（Amex 为 4-6-5） */
export function groupNumber(number: string, brand: Brand | null) {
  if (brand === 'amex' && number.length === 15) return `${number.slice(0, 4)} ${number.slice(4, 10)} ${number.slice(10)}`
  return number.replace(/(.{4})/g, '$1 ').trim()
}
