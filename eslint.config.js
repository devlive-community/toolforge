import js from '@eslint/js'
import i18next from 'eslint-plugin-i18next'
import reactHooks from 'eslint-plugin-react-hooks'
import globals from 'globals'
import tseslint from 'typescript-eslint'

/** 前端禁止引入的数据处理类库：数据处理必须在 Rust 侧完成 */
const DATA_LIBS = [
  'ajv', 'crypto-js', 'date-fns', 'dayjs', 'diff', 'fast-xml-parser', 'js-base64', 'js-yaml',
  'json5', 'jsonc-parser', 'lodash', 'lodash-es', 'moment', 'nanoid', 'papaparse', 'sql-formatter',
  'uuid', 'xml2js', 'yaml',
]

export default tseslint.config(
  { ignores: ['**/dist/**', '**/node_modules/**', 'target/**', '**/src-tauri/**', '**/*.config.{js,ts}'] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  reactHooks.configs.flat['recommended-latest'],
  {
    files: ['**/*.{ts,tsx}'],
    languageOptions: { globals: globals.browser },
    rules: {
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_', varsIgnorePattern: '^_' }],
      'no-restricted-imports': [
        'error',
        { paths: DATA_LIBS.map((name) => ({ name, message: 'Process data in Rust instead of the frontend.' })) },
      ],
      'no-restricted-globals': [
        'error',
        { name: 'localStorage', message: 'Persist through SQLite in Rust.' },
        { name: 'sessionStorage', message: 'Persist through SQLite in Rust.' },
        { name: 'indexedDB', message: 'Persist through SQLite in Rust.' },
        { name: 'atob', message: 'Decode data in Rust.' },
        { name: 'btoa', message: 'Encode data in Rust.' },
      ],
      'no-restricted-syntax': [
        'error',
        { selector: "CallExpression[callee.object.name='JSON'][callee.property.name='parse']", message: 'Parse data in Rust.' },
        { selector: "MemberExpression[object.name='crypto'][property.name='subtle']", message: 'Use Rust for cryptography.' },
      ],
    },
  },
  {
    // floating-ui 的 refs.setReference / setFloating 是回调函数而非 ref 对象，
    // 在渲染时传入是其官方用法，react-hooks/refs 会误报
    files: ['packages/ui/src/components/**/*.tsx'],
    rules: { 'react-hooks/refs': 'off' },
  },
  {
    // 界面文案必须走 i18n：JSX 文本中不得出现写死的自然语言
    files: ['apps/**/*.tsx', 'plugins/**/*.tsx'],
    plugins: { i18next },
    rules: {
      'i18next/no-literal-string': [
        'error',
        {
          mode: 'jsx-text-only',
          'should-validate-template': false,
          // 保留插件默认的数字/符号/全大写排除项；品牌与技术名词不翻译；v 为版本号前缀；
          // 快捷键与分隔符号不翻译
          words: {
            exclude: ['[0-9!-/:-@[-`{-~]+', '[A-Z_-]+', 'ToolForge', 'SQLite', 'v', '[↑↓↵·−/\\s]+'],
          },
        },
      ],
    },
  },
)
