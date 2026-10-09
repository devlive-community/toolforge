<!--
标题请使用 Conventional Commits 格式（由 CI 检查），例如：
PR titles must follow Conventional Commits (checked by CI), for example:
  feat(plugin/json-query): add a jsonpath mode
  fix(plugin/archive): decode chinese names in zip files
类型 / Types: feat · fix · perf · refactor · test · docs · chore · style · i18n · ci
描述以小写字母开头，结尾不加句号 / Start the subject in lowercase, no trailing period.
-->

## 改动说明 / Summary

<!-- 做了什么，为什么这样做 / What does this change, and why? -->

## 关联 Issue / Related issues

<!-- 例如 Closes #123 / e.g. Closes #123 -->

## 改动类型 / Type of change

- [ ] 新工具 / New tool
- [ ] 新功能或改进 / Feature or improvement
- [ ] 问题修复 / Bug fix
- [ ] 重构或性能 / Refactor or performance
- [ ] 文档、翻译或构建 / Docs, translations or build

## 已测试的平台 / Tested on

- [ ] macOS（Apple Silicon）
- [ ] macOS（Intel）
- [ ] Windows（x64）
- [ ] Linux（x64）

<!-- 只在某个平台上运行的代码（cfg(target_os)、路径分隔符、权限等）请说明在哪些平台上验证过。
     For platform-specific code (cfg(target_os), path separators, permissions…), say where it was verified. -->

## 自查清单 / Checklist

- [ ] `cargo xtask check` 全部通过 / passes
- [ ] 提交信息为英文 Conventional Commits，一个提交只做一件事 / Commits are English Conventional Commits, one change per commit
- [ ] 数据处理都在 Rust 中完成，前端只负责展示 / All data processing happens in Rust; the UI only displays results
- [ ] Rust 测试写在单独的 `*_test.rs` 文件中 / Rust tests live in separate `*_test.rs` files
- [ ] 界面文字都通过 i18n，`zh-CN` 与 `en-US` 的键一致；Rust 只返回错误码 / All UI text goes through i18n with matching `zh-CN` and `en-US` keys; Rust returns error codes only
- [ ] 样式只使用语义化 token 与 `@toolforge/ui` 组件，没有原生控件或写死的颜色 / Only semantic tokens and `@toolforge/ui` components, no native controls or hard-coded colors
- [ ] 耗时操作（约 300 ms 以上）以任务运行，可取消并显示日志 / Slow work (over ~300 ms) runs as a cancellable task with logs

新工具还需要 / For a new tool:

- [ ] `manifest.json` 中的 id 为 `org.devlive.toolforge.<name>`，分类为已有分类之一 / The id is `org.devlive.toolforge.<name>` and the category is an existing one
- [ ] 已在 `README.md` 与 `README.zh-CN.md` 的工具表中添加 / Added to the tool tables in `README.md` and `README.zh-CN.md`
- [ ] 已附上浅色与深色主题的截图 / Screenshots in light and dark themes are attached

## 截图 / Screenshots

<!-- 涉及界面的改动请附上截图或录屏 / Add screenshots or a recording for UI changes -->
