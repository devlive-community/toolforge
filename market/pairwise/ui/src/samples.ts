import type { Project } from './types'

export function initialProject(): Project {
  return {
    mode: 'form', seed: 'review-1', limits: { maxCases: 500, maxCombinations: 50000, budgetMs: 5000 },
    draft: {
      factors: [
        { name: 'OS', valuesText: '["Linux", "Windows", "macOS"]' },
        { name: 'Browser', valuesText: '["Chrome", "Firefox", "Safari"]' },
        { name: 'Role', valuesText: '["guest", "admin"]' },
      ],
      forbiddenText: '[\n  [{"factor":"OS","value":"Linux"}, {"factor":"Browser","value":"Safari"}],\n  [{"factor":"OS","value":"Windows"}, {"factor":"Browser","value":"Safari"}]\n]',
    },
    text: '{\n  "factors": [\n    {"name":"OS", "values":["Linux", "Windows", "macOS"]},\n    {"name":"Browser", "values":["Chrome", "Firefox", "Safari"]},\n    {"name":"Role", "values":["guest", "admin"]}\n  ],\n  "forbidden": [\n    [{"factor":"OS","value":"Linux"}, {"factor":"Browser","value":"Safari"}],\n    [{"factor":"OS","value":"Windows"}, {"factor":"Browser","value":"Safari"}]\n  ]\n}',
  }
}
