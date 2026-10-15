import type { Draft } from './types'

const uri = (draft: Draft) => draft === '7' ? 'http://json-schema.org/draft-07/schema#' : 'https://json-schema.org/draft/2020-12/schema'

export function sample(key: 'valid' | 'invalid' | 'refs' | 'tuple', draft: Draft) {
  const basic = `{
  "$schema": "${uri(draft)}",
  "title": "API request",
  "type": "object",
  "properties": {
    "name": { "type": "string", "minLength": 1 },
    "email": { "type": "string", "format": "email" },
    "age": { "type": "integer", "minimum": 18 }
  },
  "required": ["name", "email"],
  "additionalProperties": false
}`
  if (key === 'refs') {
    const definitions = draft === '7' ? 'definitions' : '$defs'
    return {
      schema: `{
  "$schema": "${uri(draft)}",
  "type": "object",
  "${definitions}": {
    "identifier": { "type": "string", "pattern": "^[a-z][a-z0-9_-]{2,15}$" }
  },
  "properties": {
    "owner": { "$ref": "#/${definitions}/identifier" },
    "reviewer": { "$ref": "#/${definitions}/identifier" }
  },
  "required": ["owner"]
}`,
      instance: '{\n  "owner": "alice_dev",\n  "reviewer": "Bad ID!"\n}',
    }
  }
  if (key === 'tuple') return {
    schema: `{
  "$schema": "${uri(draft)}",
  "type": "array",
  "${draft === '7' ? 'items' : 'prefixItems'}": [
    { "type": "string" },
    { "type": "integer", "minimum": 0 }
  ],
  "${draft === '7' ? 'additionalItems' : 'items'}": false,
  "minItems": 2
}`,
    instance: '["build", 42, "extra"]',
  }
  return {
    schema: basic,
    instance: key === 'valid'
      ? '{\n  "name": "Alice",\n  "email": "alice@example.com",\n  "age": 28\n}'
      : '{\n  "name": "",\n  "email": "invalid-email",\n  "age": 16,\n  "debug": true\n}',
  }
}
