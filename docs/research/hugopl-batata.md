# hugopl/batata 研究报告

## 仓库概况
- 源克隆：`/tmp/opencode/batata`
- 技术栈：Crystal language + GTK4/libadwaita + vte4 (through `hugopl/vte.cr`) on Linux.
- 定位： GNOME tiled terminal with tabs/stacks/shortcuts, desktop only.
- 版本 0.2.3, Crystal >= 1.13.3.
- 许可证 MIT.
- Application on Gtk, `Terminal < Desktop::Item`, use Vte::Terminal; desktop layout system.

## Code reading highlights
- `terminal.cr`: VTE-based UI, settings, hyperlinks, clipboards, close actions.
- `desktop/*`: stack/leaf/tab system.
- No VT parsing or rendering code: VTE system library does all.
- GTK controller bindings, keyboard shortcuts, property binding.
- `spec/` directory hosts Crystal specs; whether they execute in a current runtime was not confirmed here.
- Depends on GI-Crystal and VTE 0.78.

## Directly usable
- nil for Android; stack UI concepts can inform Compose panes.
- VTE interface API and hyperlink match-setup can guide `libghostty-vt` optional feature support.
- `match_add_regex` and hyperlink hover pattern useful.
- Popup/context menus + settings naming useful but not deep.
- application menus and exit actions as UX idea.

## Indirect useful
- desktop layout strategies (`desktop/node/layout`) in the ephemeral widget tree.
- VTE code as baseline terminal feature set; `batata` depends on vte (much older terminal) and can validate if Ghostty matches VTE independent semantics.
- Resource patterns/gio signals are irrelevant to Kotlin JNI but document event dispatch.
- Themes/GTK schema object shows settings persistent architecture.

## Slightly modifiable
- Crystal vte bindings no direct.
- Spec tests encode Crystal architecture details; runtime and syntax mismatch means they are not runnable in Torvox.
- No high-value code worth porting.

## Tests / test data
- contains `/spec` but no exhaustive test evidence.
- largely UI application behavior.

## Design patterns
- Gtk VTE is kept as child; high-level app deals with desktop tree.
- Store selection state and action enablement.
- Settings update through Gio settings.
- Close/restart state in terminal component.

## Compare torvox
- Batata is another VTE-based desktop application, offers no Android native core.
- Great for GTK desktop stack layout references only.
- Do not add VTE or Crystal branch build to Android project.
- Reuse? no.

## Conclusion
- Low relevance.

## 四次审阅记录
- 准确性: README/src/terminal.cr.
- 完整性: mostly tile UI, minimal VT code.
- 对比正确性: left as desktop.
- 文档 consistency: slug.
