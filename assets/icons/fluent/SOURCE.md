# Fluent UI System Icons: provenance

| | |
|---|---|
| Project | Microsoft Fluent UI System Icons (`microsoft/fluentui-system-icons`) |
| Package | `@fluentui/svg-icons` **1.1.344** (published 2026-10-09; git tag `1.1.344` of the repo has the same LICENSE) |
| Archive URL | https://registry.npmjs.org/@fluentui/svg-icons/-/svg-icons-1.1.344.tgz |
| Archive sha256 | `6fe61f489a6d0d065ec2182794fdadf0e15db7d3388997c5ba8039dcfaaa35e8` |
| Licence | MIT, Copyright (c) 2020 Microsoft Corporation. `package.json` says `"license": "MIT"`; the text in `LICENSE` is the repo's file at tag 1.1.344 (https://raw.githubusercontent.com/microsoft/fluentui-system-icons/1.1.344/LICENSE, sha256 `69bc45dc42b9acb96a69823adbc6ae538374e3c0bde169b855b32c48eaaef52f`). The npm package itself ships no LICENSE file. |
| Files | unmodified `icons/<name>_<20\|24>_regular.svg` from the package, copied to `svg/` (a few icons, such as window_header_horizontal, exist at 20 px only) |
| Reproduce | `scripts/fetch-fluent-icons.sh` (checks both hashes, then copies the names listed in `manifest.txt`) |
| Mapping | `manifest.txt`: command id to icon name. Appendix B of `docs/phase4/toolbar-design.md` explains the choices. |

Only the icon glyphs are used. No Microsoft Office application logo or ribbon artwork is included or imitated.

## Names (20 px and, where it exists, 24 px; regular style)

- add
- add_square
- arrow_export
- arrow_redo
- arrow_sort
- arrow_sort_down
- arrow_sort_up
- arrow_swap
- arrow_undo
- autosum
- border_all
- chevron_down
- clipboard_paste
- comment
- copy
- cut
- dark_theme
- document_landscape
- document_pdf
- edit
- filter
- highlight
- history
- home
- image
- info
- math_formula
- mic
- more_horizontal
- navigation
- orientation
- paint_brush
- paint_bucket
- play
- print
- save
- search
- select_object
- shapes
- share
- slide_add
- slide_layout
- table
- table_add
- table_cells_merge
- table_delete_column
- table_delete_row
- table_freeze_column
- table_freeze_column_and_row
- table_freeze_column_and_row_dismiss
- table_freeze_row
- table_insert_column
- table_insert_row
- text_align_center
- text_align_justify
- text_align_left
- text_align_right
- text_bold
- text_bullet_list
- text_bullet_list_tree
- text_change_case
- text_clear_formatting
- text_color
- text_font
- text_font_size
- text_grammar_checkmark
- text_indent_decrease
- text_indent_increase
- text_italic
- text_line_spacing
- text_number_format
- text_number_list_ltr
- text_paragraph
- text_proofing_tools
- text_strikethrough
- text_subscript
- text_superscript
- text_underline
- text_wrap
- textbox
- translate
- translate_auto
- window_header_horizontal
- window_header_horizontal_off
