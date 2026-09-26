import type { Field, Kind } from './types'

let counter = 0
export const newKey = () => `f${Date.now().toString(36)}${(counter++).toString(36)}`

const field = (name: string, kind: Kind, extra: Partial<Field> = {}): Field => ({ key: newKey(), name, kind, nullPercent: 0, ...extra })

export const PRESETS: Record<string, () => Field[]> = {
  users: () => [
    field('id', 'id'),
    field('name', 'name'),
    field('gender', 'gender'),
    field('age', 'age', { min: 18, max: 60 }),
    field('phone', 'phone'),
    field('email', 'email'),
    field('city', 'city'),
    field('created_at', 'datetime'),
  ],
  orders: () => [
    field('order_id', 'uuid'),
    field('customer', 'name'),
    field('amount', 'float', { min: 9.9, max: 999, decimals: 2 }),
    field('status', 'enum', { options: 'pending,paid,shipped,completed,refunded' }),
    field('address', 'address'),
    field('paid', 'boolean'),
    field('created_at', 'datetime'),
  ],
  products: () => [
    field('id', 'id', { min: 1000 }),
    field('sku', 'uuid'),
    field('name', 'sentence'),
    field('price', 'float', { min: 1, max: 5000, decimals: 2 }),
    field('stock', 'integer', { min: 0, max: 500 }),
    field('color', 'color'),
    field('description', 'paragraph', { nullPercent: 20 }),
  ],
  employees: () => [
    field('id', 'id'),
    field('name', 'name'),
    field('id_card', 'idCard'),
    field('birthday', 'birthday'),
    field('job', 'job'),
    field('company', 'company'),
    field('salary', 'integer', { min: 8000, max: 60000 }),
    field('joined_on', 'date', { from: '2018-01-01' }),
  ],
}
