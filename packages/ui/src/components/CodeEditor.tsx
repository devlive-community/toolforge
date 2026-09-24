import { forwardRef, useImperativeHandle, useMemo, useRef } from 'react'
import CodeMirror, { type ReactCodeMirrorRef } from '@uiw/react-codemirror'
import { json } from '@codemirror/lang-json'
import { sql } from '@codemirror/lang-sql'
import { markdown } from '@codemirror/lang-markdown'
import { xml } from '@codemirror/lang-xml'
import { HighlightStyle, StreamLanguage, syntaxHighlighting, type LanguageSupport } from '@codemirror/language'
import { csharp, java, kotlin } from '@codemirror/legacy-modes/mode/clike'
import { go } from '@codemirror/legacy-modes/mode/go'
import { typescript } from '@codemirror/legacy-modes/mode/javascript'
import { python } from '@codemirror/legacy-modes/mode/python'
import { rust } from '@codemirror/legacy-modes/mode/rust'
import type { Extension } from '@codemirror/state'
import { EditorSelection } from '@codemirror/state'
import { Decoration, EditorView } from '@codemirror/view'
import { tags } from '@lezer/highlight'
import { cn } from '../utils'

// 编辑器只负责展示与输入，语法高亮属于渲染层；数据处理全部在 Rust 侧
const theme = EditorView.theme({
  '&': { height: '100%', backgroundColor: 'transparent', color: 'var(--tf-fg)', fontSize: '13px' },
  '&.cm-focused': { outline: 'none' },
  '.cm-scroller': { fontFamily: 'var(--font-mono)', lineHeight: '1.65' },
  '.cm-content': { padding: '8px 0', caretColor: 'var(--tf-primary)' },
  '.cm-gutters': {
    backgroundColor: 'var(--tf-surface-2)',
    color: 'var(--tf-fg-subtle)',
    border: 'none',
    borderRight: '1px solid var(--tf-border)',
  },
  '.cm-lineNumbers .cm-gutterElement': { padding: '0 10px 0 14px', minWidth: '40px' },
  '.cm-activeLine': { backgroundColor: 'var(--tf-editor-active-line)' },
  '.cm-activeLineGutter': { backgroundColor: 'transparent', color: 'var(--tf-fg-muted)' },
  '.cm-selectionBackground, &.cm-focused .cm-selectionBackground, ::selection': {
    backgroundColor: 'var(--tf-editor-selection) !important',
  },
  '.cm-cursor': { borderLeftColor: 'var(--tf-primary)', borderLeftWidth: '2px' },
  '.cm-placeholder': { color: 'var(--tf-fg-subtle)' },
  '.cm-foldGutter .cm-gutterElement': { color: 'var(--tf-fg-subtle)' },
  '.cm-tf-error-line': { backgroundColor: 'var(--tf-editor-error-line)' },
  '.cm-tf-mark': { backgroundColor: 'var(--tf-editor-match)', borderRadius: '3px' },
  '.cm-tf-mark-alt': { backgroundColor: 'var(--tf-editor-match-alt)', borderRadius: '3px' },
  '.cm-tf-mark-warn': { backgroundColor: 'var(--tf-editor-match-warn)', borderRadius: '3px' },
  '.cm-tf-mark-active': {
    backgroundColor: 'var(--tf-editor-match-active)',
    borderRadius: '3px',
    outline: '1px solid var(--tf-primary)',
  },
  '.cm-matchingBracket': { backgroundColor: 'var(--tf-primary-soft)', outline: 'none' },
})

const highlight = syntaxHighlighting(
  HighlightStyle.define([
    { tag: tags.propertyName, color: 'var(--tf-syntax-key)' },
    { tag: tags.string, color: 'var(--tf-syntax-string)' },
    { tag: tags.number, color: 'var(--tf-syntax-number)' },
    { tag: tags.bool, color: 'var(--tf-syntax-bool)' },
    { tag: tags.null, color: 'var(--tf-syntax-null)' },
    { tag: [tags.punctuation, tags.separator, tags.brace, tags.squareBracket], color: 'var(--tf-syntax-punct)' },
    { tag: tags.comment, color: 'var(--tf-fg-subtle)', fontStyle: 'italic' },
    // XML / SQL
    { tag: [tags.tagName, tags.keyword, tags.operatorKeyword], color: 'var(--tf-syntax-key)', fontWeight: '500' },
    { tag: tags.attributeName, color: 'var(--tf-syntax-bool)' },
    { tag: tags.attributeValue, color: 'var(--tf-syntax-string)' },
    { tag: [tags.processingInstruction, tags.documentMeta], color: 'var(--tf-syntax-null)' },
    { tag: [tags.typeName, tags.standard(tags.name)], color: 'var(--tf-syntax-number)' },
    { tag: [tags.angleBracket, tags.operator], color: 'var(--tf-syntax-punct)' },
    // 代码：注解、定义名
    { tag: [tags.meta, tags.annotation], color: 'var(--tf-syntax-null)' },
    { tag: [tags.definition(tags.variableName), tags.function(tags.variableName)], color: 'var(--tf-syntax-bool)' },
    // Markdown
    { tag: tags.heading, color: 'var(--tf-syntax-key)', fontWeight: '600' },
    { tag: tags.strong, fontWeight: '600' },
    { tag: tags.emphasis, fontStyle: 'italic' },
    { tag: tags.strikethrough, textDecoration: 'line-through' },
    { tag: [tags.link, tags.url], color: 'var(--tf-primary)' },
    { tag: tags.monospace, color: 'var(--tf-syntax-string)' },
    { tag: [tags.quote, tags.contentSeparator], color: 'var(--tf-fg-muted)' },
  ]),
)

const errorLineMark = Decoration.line({ class: 'cm-tf-error-line' })
const markDecorations = {
  primary: Decoration.mark({ class: 'cm-tf-mark' }),
  alt: Decoration.mark({ class: 'cm-tf-mark-alt' }),
  warn: Decoration.mark({ class: 'cm-tf-mark-warn' }),
  active: Decoration.mark({ class: 'cm-tf-mark-active' }),
}

export type CodeLanguage =
  | 'json'
  | 'xml'
  | 'sql'
  | 'markdown'
  | 'typescript'
  | 'rust'
  | 'go'
  | 'java'
  | 'kotlin'
  | 'python'
  | 'csharp'
  | 'text'

/** 语言对应的语法高亮扩展（仅渲染用） */
function languageSupport(language: CodeLanguage): Extension | LanguageSupport | null {
  switch (language) {
    case 'json':
      return json()
    case 'xml':
      return xml()
    case 'sql':
      return sql()
    case 'markdown':
      return markdown()
    case 'typescript':
      return StreamLanguage.define(typescript)
    case 'rust':
      return StreamLanguage.define(rust)
    case 'go':
      return StreamLanguage.define(go)
    case 'java':
      return StreamLanguage.define(java)
    case 'kotlin':
      return StreamLanguage.define(kotlin)
    case 'python':
      return StreamLanguage.define(python)
    case 'csharp':
      return StreamLanguage.define(csharp)
    default:
      return null
  }
}

/** 文本高亮区间（UTF-16 偏移，与 JS 字符串下标一致） */
export interface EditorMark {
  from: number
  to: number
  tone?: keyof typeof markDecorations
}

export interface CursorPosition {
  line: number
  column: number
}

export interface CodeEditorHandle {
  focus: () => void
  gotoLine: (line: number, column?: number) => void
  /** 选中并滚动到指定区间（UTF-16 偏移） */
  selectRange: (from: number, to: number) => void
  /** 用前后缀包裹选区；选区已被包裹时取消包裹，无选区时插入前后缀并把光标放在中间 */
  wrapSelection: (before: string, after?: string) => void
  /** 为选区所在的每一行切换行首前缀；`replace` 匹配到的已有前缀会被替换 */
  toggleLinePrefix: (prefix: string, replace?: RegExp) => void
  /** 用文本替换选区，光标移到插入内容之后 */
  insertText: (text: string) => void
}

export interface CodeEditorProps {
  value: string
  onChange?: (value: string) => void
  language?: CodeLanguage
  readOnly?: boolean
  lineWrapping?: boolean
  placeholder?: string
  /** 需要标红的行（从 1 开始） */
  errorLine?: number | null
  /** 高亮区间，需按 from 升序 */
  marks?: EditorMark[]
  onCursorChange?: (position: CursorPosition) => void
  className?: string
  'aria-label'?: string
}

export const CodeEditor = forwardRef<CodeEditorHandle, CodeEditorProps>(function CodeEditor(
  { value, onChange, language = 'json', readOnly, lineWrapping, placeholder, errorLine, marks, onCursorChange, className, ...aria },
  ref,
) {
  const cm = useRef<ReactCodeMirrorRef>(null)

  useImperativeHandle(ref, () => ({
    focus: () => cm.current?.view?.focus(),
    gotoLine: (line, column = 1) => {
      const view = cm.current?.view
      if (!view) return
      const target = view.state.doc.line(Math.min(Math.max(line, 1), view.state.doc.lines))
      const pos = Math.min(target.from + Math.max(column - 1, 0), target.to)
      view.dispatch({ selection: EditorSelection.cursor(pos), scrollIntoView: true })
      view.focus()
    },
    selectRange: (from, to) => {
      const view = cm.current?.view
      if (!view) return
      const length = view.state.doc.length
      view.dispatch({
        selection: EditorSelection.range(Math.min(from, length), Math.min(to, length)),
        scrollIntoView: true,
      })
    },
    wrapSelection: (before, after = before) => {
      const view = cm.current?.view
      if (!view) return
      view.dispatch(
        view.state.changeByRange((range) => {
          const text = view.state.sliceDoc(range.from, range.to)
          const wrapped = text.length >= before.length + after.length && text.startsWith(before) && text.endsWith(after)
          const inner = wrapped ? text.slice(before.length, text.length - after.length) : before + text + after
          const start = range.empty ? range.from + before.length : range.from
          const end = range.empty ? start : range.from + inner.length
          return { changes: { from: range.from, to: range.to, insert: inner }, range: EditorSelection.range(start, end) }
        }),
      )
      view.focus()
    },
    toggleLinePrefix: (prefix, replace) => {
      const view = cm.current?.view
      if (!view) return
      const { state } = view
      const lines = new Map<number, ReturnType<typeof state.doc.line>>()
      for (const range of state.selection.ranges) {
        for (let n = state.doc.lineAt(range.from).number; n <= state.doc.lineAt(range.to).number; n++) lines.set(n, state.doc.line(n))
      }
      const all = [...lines.values()]
      const remove = all.every((line) => line.text.startsWith(prefix))
      const changes = all.map((line) => {
        if (remove) return { from: line.from, to: line.from + prefix.length, insert: '' }
        const existing = replace ? (line.text.match(replace)?.[0] ?? '') : ''
        return { from: line.from, to: line.from + existing.length, insert: prefix }
      })
      view.dispatch({ changes })
      view.focus()
    },
    insertText: (text) => {
      const view = cm.current?.view
      if (!view) return
      view.dispatch(view.state.replaceSelection(text), { scrollIntoView: true })
      view.focus()
    },
  }))

  const extensions = useMemo(() => {
    const list = [theme, highlight]
    const support = languageSupport(language)
    if (support) list.push(support)
    if (lineWrapping) list.push(EditorView.lineWrapping)
    if (errorLine && errorLine > 0) {
      list.push(
        EditorView.decorations.compute(['doc'], (state) => {
          if (errorLine > state.doc.lines) return Decoration.none
          return Decoration.set([errorLineMark.range(state.doc.line(errorLine).from)])
        }),
      )
    }
    if (marks && marks.length > 0) {
      list.push(
        EditorView.decorations.compute(['doc'], (state) => {
          // 编辑过程中高亮可能对应旧文本：超出文档的区间截断或跳过
          const length = state.doc.length
          const ranges = marks
            .filter((mark) => mark.from < length && mark.to > mark.from)
            .map((mark) => markDecorations[mark.tone ?? 'primary'].range(mark.from, Math.min(mark.to, length)))
          return Decoration.set(ranges, true)
        }),
      )
    }
    return list
  }, [language, lineWrapping, errorLine, marks])

  return (
    <CodeMirror
      ref={cm}
      value={value}
      onChange={onChange}
      readOnly={readOnly}
      placeholder={placeholder}
      extensions={extensions}
      theme="none"
      basicSetup={{
        highlightActiveLine: !readOnly,
        highlightActiveLineGutter: true,
        foldGutter: true,
        autocompletion: false,
        searchKeymap: true,
        lintKeymap: false,
      }}
      onUpdate={(update) => {
        if (!onCursorChange || !(update.selectionSet || update.docChanged)) return
        const head = update.state.selection.main.head
        const line = update.state.doc.lineAt(head)
        onCursorChange({ line: line.number, column: head - line.from + 1 })
      }}
      className={cn('h-full min-h-0 overflow-hidden', className)}
      {...aria}
    />
  )
})
