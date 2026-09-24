import { forwardRef, useImperativeHandle, useMemo, useRef } from 'react'
import CodeMirror, { type ReactCodeMirrorRef } from '@uiw/react-codemirror'
import { json } from '@codemirror/lang-json'
import { sql } from '@codemirror/lang-sql'
import { xml } from '@codemirror/lang-xml'
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language'
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
  ]),
)

const errorLineMark = Decoration.line({ class: 'cm-tf-error-line' })
const markDecorations = {
  primary: Decoration.mark({ class: 'cm-tf-mark' }),
  alt: Decoration.mark({ class: 'cm-tf-mark-alt' }),
  warn: Decoration.mark({ class: 'cm-tf-mark-warn' }),
  active: Decoration.mark({ class: 'cm-tf-mark-active' }),
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
}

export interface CodeEditorProps {
  value: string
  onChange?: (value: string) => void
  language?: 'json' | 'xml' | 'sql' | 'text'
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
  }))

  const extensions = useMemo(() => {
    const list = [theme, highlight]
    if (language === 'json') list.push(json())
    else if (language === 'xml') list.push(xml())
    else if (language === 'sql') list.push(sql())
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
