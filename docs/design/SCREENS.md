# andre — screen pattern, mockups, mouse, build plan (designer spec; workers do not edit)

> Implemented. Kept as design rationale; see docs/design/README.md.

Companion to `FEATURES.md`. Read `AGENTS.md` first. Mockups are 80x24, unicode icon set,
generated so every frame is exactly 80 columns (the cursor row's `selection` fill can't be
drawn in ASCII; `▌` marks it). Content area at 80x24 is **76 x 16** on inner screens and
**76 x 10** on the main menu (big banner).

## 0. Decisions (one per question)

| # | Decision | Why |
|---|----------|-----|
| D1 | Every screen is **list + detail** (two-pane). No single-pane document screens remain except the wizard's review step and the too-small overlay. | One grammar to learn; the detail pane replaces legends, help paragraphs and truncated columns. |
| D2 | **Legend panels are removed** (packages, status, file browsers). Status meaning lives in the detail headline (`◐ partial — some files linked`) and in help › status glyphs. | They cost 5–8 of 16 rows at 80x24. |
| D3 | **Group select + package select merge** into one grouped packages browser (`▾ group` rows with packages under them). `GroupSelectComponent` is deleted. | One screen instead of two; marks across groups; the detail pane carries what the second screen showed. |
| D4 | **Package scanning splash is removed.** Unscanned packages show `·` (unknown) and fill in as `StatusCache` answers. | Never block the screen for a second. |
| D5 | **Screen verbs avoid the global letters** `v d n a o s t q`. New = `+`, edit = `e`, rename = `r`, delete = `x`, save = `w`, filter = `f`, check status = `c`, ignore = `i`, profiles = `p`, rescan = `r` (status only, no rename there). **Spec change:** FEATURES.md says `n` for new profile; `n` is the global no-fold key and component-first dispatch would make the top-bar `no-fold` chip dead on that screen. Orchestrator: accept `+`. | Global chips must work on every Normal-mode screen. |
| D6 | **Files change → review screen** (two-pane + action bar, `y`/`n`). **Config/profile change → dialog** (small overlay, buttons) or an edit screen saved with `w`. | One rule decides confirm vs dialog. |
| D7 | Edit screens (profile edit, settings, ignore editor) keep a pending copy; `w` saves; `Esc` with unsaved edits opens the **leave dialog** (`y save` / `n discard` / `Esc stay`). Settings' `[Update andre.yml]` row goes away. | Same save story everywhere. |
| D8 | **Bootstrap** "set up this machine" is item `0`, placed first. With 9 items the main menu doesn't fit under the big banner at 24 rows, so the main menu switches to the one-row title whenever `items + 2 > rows left under the big banner`. | Visible first thing on a fresh machine; no special layout. |
| D9 | Mouse: **click selects; click on the already-selected row activates** (= its primary verb). No double-click timer — a double-click is just two clicks and does the same. In multi-select lists the primary verb of a row is *mark* (Space), never *run*. | Deterministic, testable without timing; can't launch a stow by accident. |
| D10 | Mouse capture is always on. Help › mouse tells users to hold Shift to select terminal text. No setting, no flag. | Smallest surface. |
| D11 | Breadcrumb is lowercase with ` › ` and is not clickable (v1). | Popping several screens past pending edits needs the leave dialog per screen; not worth it. |

## 1. The screen pattern

### 1.1 Frame (unchanged chrome)

```
row  0     ╭── frame border ──╮
row  1     title: big banner (main menu, 7 rows) or `andre  main › packages` (1 row)
row  2     separator
row  3     top bar: action pill + verbose meter │ flag chips      (clickable chips)
row  4     separator
rows 5–20  CONTENT 76x16  (main menu: 76x10)
row 21     separator
row 22     footer: screen hints …… theme chip  q quit            (clickable hints)
row 23     ╰──────────────────╯
```

### 1.2 Layout grammar

All content layouts are built by one helper (`ui/layout.rs`, §1.9). Heights below are at 80x24.

| Layout | Rows | Used by |
|--------|------|---------|
| **A. two-pane** | list \| detail, 16 rows | main menu (10), profiles, packages, unstow targets/browse, settings, help |
| **B. chip row + two-pane** | 1 chip row, panes 15 | status (profile filter + summary), ignore editor (file tabs), execute (gauge) |
| **C. wizard** | step bar, blank, panes 14 | new-package steps 1–5 |
| **D. review** | panes 15, action bar 1 | stow/restow/unstow review, unstow-links review, execute done (chip row + panes 14 + bar) |
| **E. wide panel** | one panel full width + action bar | wizard step 6 review only (lines are paths, need the width) |
| **F. dialog** | overlay centred on content | new/rename/delete profile, membership, leave dialog, settings group edit/remove/browse, pickers |
| **F′. dropdown** | overlay anchored under its chip | status profile filter |

Pane widths: `list = clamp(content_width * 2 / 5, 34, 48)`, gap 1, detail = rest.
80 cols → 34 | 41 (today's main menu); 120 cap → 48 | 71. Content stays capped at 120 and
centred (`MAX_CONTENT_WIDTH`, unchanged).

Dialog size: width = widest line + 4, clamped to `[30, min(60, content_width - 4)]`; height =
lines + 2; centred in the content area. A dialog never covers the top bar or footer.

### 1.3 Titles and breadcrumb

- Title row: `andre  main › packages › home/shell`. All lowercase, ` › ` separator. Each
  component returns its crumb (`fn crumb(&self) -> Cow<str>`, default = today's
  `breadcrumb_label(id)` lowercased); entity screens use the entity (`personal`,
  `home/shell`). Wider than the row → drop crumbs from the left behind `… › `.
- **List panel title** = collection noun + count: `profiles 3`, `packages 9`; when the list
  has marks: `packages 2 of 9` (marked of total); a filtered list uses the filter:
  `agent-work 5`. Unsaved edits append ` · unsaved ●` (`●` in `warn`).
- **Detail panel title** = the selected item's name (`home/shell`, `agent-work`, `verbose`).
- Panel titles never repeat the breadcrumb (existing test keeps this).

### 1.4 Hints (footer)

- Format `key verb`, two spaces between hints, key in `key`, verb in `text_dim` (as today).
- Order: primary (`Enter …`), mark (`Space mark`), screen verbs (`+ e r x i p c f w`),
  movement extras (`←→ fold`, `Tab pane`), `Esc back` last.
- `↑↓ move` appears only on screens with ≤ 3 other hints (it's universal).
- The primary verb says what happens, with counts and the live action:
  `Enter stow 3`, `Enter restow`, `y remove 2`.
- Hints become data: `Hint { key: KeyEvent, label: Cow<'static, str>, prio: u8 }`. When the
  row is too narrow the footer drops the lowest-prio hints whole (theme chip first, as
  today), never cuts a word. `q quit` is always kept. Each hint is clickable (§3).

### 1.5 Rows, marks and glyphs

Column order inside a row (after the selection painter's 1-col gutter / `▌`):

```
[fold] [mark] [status] [kind] name ………… right meta (1 col before the border)
 ▾▸     ■□▣    ✓→◐✗?∅·   /~•          `via base`, `3/4`, `12`, digit
```

| Role | Unicode | Nerd | Colour |
|------|---------|------|--------|
| cursor bar | `▌` | `▌` | `accent` (shared painter, `ui/selection.rs`) |
| mark on / off | `■` / `□` | existing | on `accent`, off `text_dim` |
| mark some (group header, inherited) **NEW** `checkbox_some` | `▣` | `\u{f147}` | `accent` |
| radio on / off **NEW** `radio_on/off` | `●` / `○` | `\u{f192}` / `\u{f10c}` | on `accent`, off `text_dim` |
| fold open / closed **NEW** `fold_open/closed` | `▾` / `▸` | `\u{f0d7}` / `\u{f0da}` | `text_dim` |
| status ✓ → ◐ ✗ ? ∅ | existing `Icons::status` | existing | `status_color` |
| status unknown **NEW** `unknown` | `·` | `\u{f141}` | `text_dim` |
| kind dir / symlink / file | `/` `~` `•` | existing browser glyphs | `text_dim` |
| menu: profiles **NEW** | `◈` | `\u{f0e8}` | `accent` |
| menu: new package **NEW** | `+` | `\u{f067}` | `accent` |
| menu: set up machine **NEW** | `⌂` | `\u{f108}` | `accent` |
| menu: packages / unstow / status / settings / help / quit | existing `link unlink check gear question power` | | `accent` |

All new glyphs are one column wide (`unicode-width`); add them to `icons.rs`'
`all_glyphs` width test. The selection painter's 3:1 glyph rule applies to every glyph
(pass glyphs through `row_with_glyphs`, never as content spans).

Right meta: `text_dim`; counts `value`. Inherited rows (package comes via an include) show a
`text_dim` `■` plus `via <profile>` and cannot be toggled (Space shows a toast:
`comes from base — edit base or drop the include`).

### 1.6 Spacing and text

- Panel inner gutter 1 col (painter). Detail text inset 1 col (existing test).
- Detail order: **headline** (1–2 lines; status headline starts with its glyph), blank,
  **facts** (label column 10, `text_dim`; value `value`), blank, **section** (a short list:
  files, packages, patterns). No blank first row. One blank line between blocks, never two.
- Paths always via the existing `display_path` (`~` under home), elided at the end with `…`.
- Tone: lowercase labels, sentences in sentence case, no `Press 'y' to …` lines inside
  panels (the footer and buttons say it).

### 1.7 States

- **Empty**: the list panel shows a `text_dim` sentence centred vertically plus one hint
  line (`+ new profile`) that is clickable; the detail pane explains the concept.
- **Loading**: no splash. Rows render immediately; unknown status = `·`; the chip row (or
  detail facts) shows `scanning 4/9 ⣼` using the execute spinner frames.
- **Error**: toast (`show_error_toast`) for failed actions; config errors that block a
  screen render as the detail headline in `err` with the list still navigable.
- **Progress**: chip row = `<action>  ████░░░░  done/total  spinner`; ratatui `Gauge` is fine.

### 1.8 Sizes

- 80x24 is the design size (all mockups). Extra rows go to the panes (lists show more
  rows; detail sections show more lines). Extra columns go to the list pane up to 48, then
  to the detail pane, up to the 120-col cap.
- The list pane adds a right-aligned status word column when it is ≥ 44 cols wide
  (`✓ git        stowed`); below that the word lives only in the detail headline.
- Main menu: big banner only when `rows_under_big_banner >= items + 2` (D8).
- Below 80x24: the too-small overlay (§2.15).

### 1.9 Shared widgets (new modules, API sketches)

```rust
// ui/layout.rs — every screen's rects, computed in prepare()
pub struct ScreenRects { pub chip: Option<Rect>, pub list: Rect, pub detail: Rect, pub actions: Option<Rect> }
pub enum Shape { TwoPane, ChipTwoPane, Wizard, Review, ChipReview, Wide }
pub fn screen_rects(area: Rect, shape: Shape) -> ScreenRects;
pub fn list_width(content_width: u16) -> u16;            // clamp(w*2/5, 34, 48)

// ui/list_panel.rs — scrolling list in a themed panel, rows painted by ui/selection.rs
#[derive(Default)] pub struct ListView { pub cursor: usize, pub offset: usize, inner: Rect }
impl ListView {
    pub fn prepare(&mut self, panel: Rect, len: usize);   // keeps cursor visible, stores inner rect
    pub fn row_at(&self, x: u16, y: u16) -> Option<usize>; // mouse hit test (uses stored rect)
    pub fn step(&mut self, down: bool, len: usize);        // wraps (input::step_cursor)
    pub fn scroll(&mut self, lines: i16, len: usize);      // wheel
}
pub struct Empty<'a> { pub message: &'a str, pub hint: Option<(&'a str, &'a str)> }
pub fn render_list_panel(f: &mut Frame, area: Rect, c: &ThemeColors, title: &str,
                         rows: Vec<Line<'static>>, view: &ListView, empty: Option<Empty>);

// ui/tree.rs — grouped package rows shared by packages, profile edit, status, review
pub enum Mark { On, Off, Some, Inherited(String) }
pub enum TreeRow { Group { name: String, folded: bool, mark: Option<Mark>, meta: String },
                   Package { group: String, name: String, status: Option<StowStatus>, mark: Option<Mark>, meta: String } }
pub struct Tree { groups: Vec<TreeGroup>, folded: HashSet<String> }
impl Tree { pub fn rows(&self) -> Vec<TreeRow>; pub fn toggle_mark(&mut self, row: usize);
            pub fn fold(&mut self, row: usize, folded: bool); pub fn marked(&self) -> Vec<(String, String)>; }
pub fn tree_line(row: &TreeRow, selected: bool, icons: &Icons, c: &ThemeColors, width: u16) -> Line<'static>;

// ui/detail.rs — generalises components::menu::Detail; built in update(), drawn from &self
pub struct Detail { pub headline: Vec<Line<'static>>, pub facts: Vec<(Cow<'static, str>, Line<'static>)>,
                    pub sections: Vec<Line<'static>>, pub scroll: u16 }
pub fn render_detail(f: &mut Frame, area: Rect, c: &ThemeColors, title: &str, d: &Detail);

// ui/dialog.rs — overlays and the action bar share Button
pub struct Button { pub key: KeyEvent, pub label: Cow<'static, str>, pub primary: bool }
pub struct Dialog { pub title: String, pub body: Vec<Line<'static>>, pub buttons: Vec<Button> }
pub fn dialog_rect(content: Rect, d: &Dialog) -> Rect;
pub fn render_dialog(f: &mut Frame, rect: Rect, c: &ThemeColors, d: &Dialog); // Clear + panel
pub fn render_action_bar(f: &mut Frame, area: Rect, c: &ThemeColors, buttons: &[Button]); // right-aligned
pub fn button_rects(area: Rect, buttons: &[Button]) -> Vec<(Rect, KeyEvent)>;          // mouse
// Button look: `[ key label ]`; primary = `on_accent` on `accent` fill (like the action pill), rightmost.

// ui/step_bar.rs
pub struct StepBar { pub steps: &'static [&'static str], pub current: usize }
pub fn step_line(s: &StepBar, c: &ThemeColors) -> Line<'static>;  // done `✓ name` ok, current `[n name]` accent bold, next text_dim
pub fn step_rects(area: Rect, s: &StepBar) -> Vec<(Rect, usize)>;

// ui/text_field.rs — one-line input (profile name, package name, pattern, description)
pub struct TextField { pub value: String, pub cursor: usize, pub error: Option<String> }
impl TextField { pub fn handle_key(&mut self, k: &KeyEvent) -> bool; } // chars, Backspace, Delete, ←→, Home/End
pub fn field_line(label: &str, f: &TextField, focused: bool, c: &ThemeColors) -> Line<'static>; // cursor = reversed cell

// ui/chips.rs — tab/filter row above panes
pub struct Chip { pub label: String, pub active: bool, pub key: KeyEvent }
pub fn chip_line(chips: &[Chip], right: Vec<Span<'static>>, c: &ThemeColors, width: u16) -> Line<'static>;
pub fn chip_rects(area: Rect, chips: &[Chip]) -> Vec<(Rect, KeyEvent)>;
```

Existing pieces reused as-is: `ui/selection.rs` (all cursor rows), `status_style`
(`status_color`, `status_word`), `themed_panel`, `widgets/scrollbar.rs` (list + detail),
`FileBrowserWidget` (rendered inside a `ListView` panel), `Browser`, `Icons`, toast, top bar,
`execute` spinner frames.

## 2. Screens (80x24, unicode icons)

### 2.1 Main menu (8 items)

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│                                                                              │
│                    ╔════╗          ╔════╗          ╔════╗                    │
│                    ║ A. ║  ╔════╗  ║ D. ║  ╔════╗  ║ E. ║                    │
│                    ╚════╝  ║ N. ║  ╚════╝  ║ R. ║  ╚════╝                    │
│                            ╚════╝          ╚════╝                            │
│                                                                              │
│                    Another Neat Dotfile Repository Engine                    │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ actions ───────────────────────╮ ╭ profiles ─────────────────────────────╮ │
│ │▌◈  profiles                  1 │ │ Named sets of packages. Pick one and  │ │
│ │ ⇢  packages                  2 │ │ run the current action on all of it.  │ │
│ │ +  new package               3 │ │                                       │ │
│ │ ≠  unstow                    4 │ │ profiles  3                           │ │
│ │ ✓  verify status             5 │ │ packages  9 in 3 groups               │ │
│ │ ≡  settings                  6 │ │ stowed    6 of 9                      │ │
│ │ ?  help                      7 │ │                                       │ │
│ │ ×  quit                      8 │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ ↑↓ move  1–8 jump  Enter open                          theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Items/digits: `1 profiles  2 packages  3 new package  4 unstow  5 verify status  6 settings
7 help  8 quit`. Detail facts per item (built in `update`, as today): profiles → count,
packages, stowed; packages → groups, packages, stowed; new package → groups, targets;
unstow → targets; status → summary if cached; settings → config file, engine, theme;
help → key reminder; quit → none. Enter / click-on-selected opens.

### 2.2 Main menu with bootstrap item (D8)

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main                                                                  │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ actions ───────────────────────╮ ╭ set up this machine ──────────────────╮ │
│ │▌⌂  set up this machine       0 │ │ This config folder lives in a repo.   │ │
│ │ ◈  profiles                  1 │ │ Link it to ~/.config/andre so plain   │ │
│ │ ⇢  packages                  2 │ │ `andre` finds it next time.           │ │
│ │ +  new package               3 │ │                                       │ │
│ │ ≠  unstow                    4 │ │ package   config/andre                │ │
│ │ ✓  verify status             5 │ │ links     ~/.config/andre             │ │
│ │ ≡  settings                  6 │ │ from      ~/src/dotfiles/config       │ │
│ │ ?  help                      7 │ │ action    stow (always)               │ │
│ │ ×  quit                      8 │ │                                       │ │
│ │                                │ │ Enter reviews the link first.         │ │
│ │                                │ │ Shown until ~/.config/andre exists.   │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ ↑↓ move  0–8 jump  Enter open                          theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Enter → the normal review screen (2.13) for exactly that package with action `stow`
regardless of the global action → execute. The item disappears once `~/.config/andre` exists.

### 2.3 Profiles (list + detail)

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › profiles                                                       │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ profiles 3 ────────────────────╮ ╭ agent-work ───────────────────────────╮ │
│ │▌◈ agent-work                 5 │ │ Work servers that run agents          │ │
│ │ ◈ base                       3 │ │                                       │ │
│ │ ◈ personal                   7 │ │ includes  base                        │ │
│ │                                │ │ packages  5  (2 own, 3 via base)      │ │
│ │                                │ │ status    ✓ 3  → 1  ✗ 1               │ │
│ │                                │ │                                       │ │
│ │                                │ │ ✓ work/aws                            │ │
│ │                                │ │ → work/kube                           │ │
│ │                                │ │ ✓ home/git              via base      │ │
│ │                                │ │ ✓ home/shell            via base      │ │
│ │                                │ │ ✗ config/andre          via base      │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Enter stow  + new  e edit  r rename  x delete  c check         theme  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

- Row: `◈ name` + resolved package count right. Detail: description headline, facts
  (includes, packages own/via, status summary), section = resolved packages with status
  glyph and `via <profile>` for included ones (scrolls with the wheel; last visible line
  becomes `+ N more` when it overflows).
- Enter = run the current action on the whole profile → review (2.13). `c` = status
  filtered to this profile (2.15b). `+` new, `e` edit, `r` rename, `x` delete.
- Missing refs show `?` and `missing` meta; a config error (cycle, unknown include) is the
  detail headline in `err` and Enter is disabled (footer drops `Enter`).

Empty:

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › profiles                                                       │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ profiles 0 ────────────────────╮ ╭ profiles ─────────────────────────────╮ │
│ │                                │ │ A profile is a named set of packages, │ │
│ │                                │ │ e.g. `personal` or `agent-work`.      │ │
│ │                                │ │ Profiles can include other profiles.  │ │
│ │                                │ │                                       │ │
│ │      no profiles yet           │ │ Saved in ~/.config/andre/profiles/.   │ │
│ │                                │ │                                       │ │
│ │    + new profile               │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ + new  Esc back                                        theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

New (rename is the same dialog titled `rename profile`, prefilled):

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › profiles                                                       │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ profiles 3 ────────────────────╮ ╭ agent-work ───────────────────────────╮ │
│ │▌◈ agent-work                 5 │ │ Work servers that run agents          │ │
│ │ ◈ base                       3 │ │                                       │ │
│ │ ◈ personal                   7 │ │                                       │ │
│ │               ╭ new profile ─────────────────────────────╮               │ │
│ │               │                                          │               │ │
│ │               │  name  laptop█                           │               │ │
│ │               │        a-z 0-9 _ -, starts a-z/0-9       │               │ │
│ │               │                                          │               │ │
│ │               │         [ Esc cancel ]  [ Enter create ] │               │ │
│ │               ╰──────────────────────────────────────────╯               │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Enter create  Esc cancel                               theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Invalid names keep the button disabled (`text_dim`) and show the rule line in `warn`;
duplicates say `already exists`.

Delete:

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › profiles                                                       │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ profiles 3 ────────────────────╮ ╭ base ─────────────────────────────────╮ │
│ │ ◈ agent-work                 5 │ │ Shared by every machine               │ │
│ │▌◈ base                       3 │ │                                       │ │
│ │ ◈ personal                   7 │ │                                       │ │
│ │            ╭ delete profile ────────────────────────────────╮            │ │
│ │            │                                                │            │ │
│ │            │  Delete profile `base`?                        │            │ │
│ │            │  It is included by: agent-work, personal.      │            │ │
│ │            │  Those includes are removed too. No files      │            │ │
│ │            │  are unlinked.                                 │            │ │
│ │            │                       [ n keep ]  [ y delete ] │            │ │
│ │            ╰────────────────────────────────────────────────╯            │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ y delete  n keep                                       theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

### 2.4 Profile edit

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › profiles › personal                                            │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ packages 3 of 9 ───────────────╮ ╭ personal · edited ● ──────────────────╮ │
│ │ ▾ ▣ home                   3/4 │ │ description                           │ │
│ │     ■ ✓ git          via base  │ │  Personal laptop█                     │ │
│ │     ■ ✓ shell        via base  │ │                                       │ │
│ │▌    ■ ✓ ssh                    │ │ includes                              │ │
│ │     □ → tmux                   │ │  ■ base                               │ │
│ │ ▾ □ config                 0/3 │ │  □ agent-work                         │ │
│ │     □ ✓ andre                  │ │                                       │ │
│ │     □ → kitty                  │ │ resolved  3 packages                  │ │
│ │     □ ✓ nvim                   │ │ own       1  ssh                      │ │
│ │ ▸ □ work                   0/2 │ │ via base  2  git, shell               │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Tab pane  Space toggle  w save  Esc back               theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

- Left: the shared grouped tree (`ui/tree.rs`) of all packages with marks; inherited rows
  are `■ … via base` in `text_dim`, not toggleable. Group header mark: `■` all own, `□`
  none, `▣` some; Space on a header marks/unmarks all its non-inherited packages.
- Right: form pane. Rows: `description` (Enter edits inline → `InputMode::TextInput`,
  Enter/Esc leaves the field), `includes` checkbox rows (all other profiles; choosing one
  that would make a cycle is refused with a toast naming the cycle), then live facts
  (resolved / own / via).
- `Tab` switches the focused pane (focused pane border in `accent`, other in `border`).
  `w` saves; Esc with edits → leave dialog:

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › profiles › personal                                            │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ packages 3 of 9 ───────────────╮ ╭ personal · edited ● ──────────────────╮ │
│ │ ▾ ▣ home                   3/4 │ │ description                           │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │              ╭ unsaved changes ───────────────────────────╮              │ │
│ │              │                                            │              │ │
│ │              │  Save changes to profile `personal`?       │              │ │
│ │              │                                            │              │ │
│ │              │                                            │              │ │
│ │              │      [ Esc stay ] [ n discard ] [ y save ] │              │ │
│ │              ╰────────────────────────────────────────────╯              │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ y save  n discard  Esc stay                            theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

### 2.5 Packages (merged browser, D3)

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › packages                                                       │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ packages 1 of 9 ───────────────╮ ╭ home/shell ───────────────────────────╮ │
│ │ ▾ ▣ home                   1/4 │ │ ◐ partial — some files linked         │ │
│ │     □ ✓ git                    │ │                                       │ │
│ │▌    ■ ◐ shell                  │ │ group     home  →  ~                  │ │
│ │     □ ✓ ssh                    │ │ source    ~/src/dotfiles/home/shell   │ │
│ │     □ → tmux                   │ │ files     4  (2 linked)               │ │
│ │ ▾ □ config                 0/3 │ │ ignore    .gitignore ✓  stow ✗        │ │
│ │     □ ✗ andre                  │ │ profiles  base, personal              │ │
│ │     □ → kitty                  │ │                                       │ │
│ │     □ ✓ nvim                   │ │ ✓ .bashrc                             │ │
│ │ ▸ □ work                   0/2 │ │ ✓ .profile                            │ │
│ │                                │ │ → .zshrc                              │ │
│ │                                │ │ → .config/starship.toml               │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Space mark  Enter stow 1  i ignore  p profiles  ←→ fold        theme  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Group row selected:

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › packages                                                       │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ packages 1 of 9 ───────────────╮ ╭ home ─────────────────────────────────╮ │
│ │▌▾ ▣ home                   1/4 │ │ Group of 4 packages.                  │ │
│ │     □ ✓ git                    │ │                                       │ │
│ │     ■ ◐ shell                  │ │ source    ~/src/dotfiles/home         │ │
│ │     □ ✓ ssh                    │ │ target    ~                           │ │
│ │     □ → tmux                   │ │ status    ✓ 2  ◐ 1  → 1               │ │
│ │ ▾ □ config                 0/3 │ │ marked    1 of 4                      │ │
│ │     □ ✗ andre                  │ │                                       │ │
│ │     □ → kitty                  │ │ Space marks every package in the      │ │
│ │     □ ✓ nvim                   │ │ group; again unmarks them.            │ │
│ │ ▸ □ work                   0/2 │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Space mark  Enter stow 1  i ignore  p profiles  ←→ fold        theme  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

- Rows: fold, mark, status, name. Group rows: fold, tri-state mark, name, `marked/total`.
  `←` folds (on a package: jumps to its group), `→` unfolds. Folded state is per session.
- Enter: run the current action on the marked packages; with no marks, on the cursor
  package (or the whole group on a group row) → review. Footer verb shows the count.
- Detail (package): status headline, group → target, source, files (linked count),
  ignore files present, profiles, then a file section with per-file status (from the
  existing status scan). Detail (group): source, target, status summary, marked count.
- `i` ignore editor (2.6) for the cursor package, `p` profile membership dialog:

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › packages                                                       │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ packages 1 of 9 ───────────────╮ ╭ home/shell ───────────────────────────╮ │
│ │ ▾ ▣ home                   1/4 │ │ ◐ partial — some files linked         │ │
│ │                                │ │                                       │ │
│ │                 ╭ home/shell in profiles ──────────────╮                 │ │
│ │                 │▌■ base                               │                 │ │
│ │                 │ ■ personal                           │                 │ │
│ │                 │ □ agent-work                         │                 │ │
│ │                 │                                      │                 │ │
│ │                 │ Space toggle                         │                 │ │
│ │                 │       [ Esc cancel ]  [ Enter save ] │                 │ │
│ │                 ╰──────────────────────────────────────╯                 │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Space toggle  Enter save  Esc cancel                   theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

### 2.6 Package ignore editor

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › packages › home/shell › ignore                                 │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  [.gitignore 8]  .stow-local-ignore 3                                        │
│ ╭ .gitignore ────────────────────╮ ╭ *.env ────────────────────────────────╮ │
│ │ # secrets                      │ │ Git ignores matching files. andre     │ │
│ │▌*.env                          │ │ never reads this file for linking.    │ │
│ │ .env*                          │ │                                       │ │
│ │ *history*                      │ │ pattern   *.env                       │ │
│ │ *.key                          │ │ matches   2 files in this package     │ │
│ │ *.pem                          │ │                                       │ │
│ │ secrets/                       │ │ · work.env                            │ │
│ │ .cache/                        │ │ · .config/app/prod.env                │ │
│ │                                │ │                                       │ │
│ │  + add a pattern               │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Tab file  + add  e edit  x delete  w save  Esc back    theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › packages › home/shell › ignore                                 │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  .gitignore 8  [.stow-local-ignore 3]                                        │
│ ╭ .stow-local-ignore ────────────╮ ╭ ^/README.* ───────────────────────────╮ │
│ │ # never link these             │ │ Files that match are never linked.    │ │
│ │▌^/README.*                     │ │ One Perl regex per line.              │ │
│ │ \.git                          │ │                                       │ │
│ │                                │ │ ! This file REPLACES the default      │ │
│ │  + add a pattern               │ │   ignore list for this package.       │ │
│ │                                │ │                                       │ │
│ │                                │ │ pattern   ^/README.*                  │ │
│ │                                │ │ matches   1 file                      │ │
│ │                                │ │                                       │ │
│ │                                │ │ · README.md                           │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Tab file  + add  e edit  x delete  w save  Esc back    theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

- Chip row = file tabs with line counts; `Tab` or click switches. The file that does not
  exist yet shows `(none)` and `+` creates it.
- Rows are file lines; comments `#…` in `text_dim`. `+` appends a new row in edit mode
  (TextField inline in the list), `e`/Enter edits, `x` deletes, `w` saves both files, Esc →
  leave dialog when dirty.
- Detail: what the file does (`.stow-local-ignore` carries the *replaces the defaults*
  warning in `warn`), the pattern, and the files in the package it matches (computed in
  `update` with the same matcher the engine uses; `.gitignore` uses simple glob matching
  for preview only and says so).
- The same component edits the settings' global ignore list and a group's ignore list
  (no tabs then, title `ignore · global` / `ignore · group home`).

### 2.7 New-package wizard

Step bar on top (`StepBar`); steps 1–5 are two-pane; Enter = next, Esc = previous step
(on step 1: leave, with the leave dialog if anything was chosen). Completed steps in the
bar are clickable to jump back. State is one `NewPackageWizard` component with a `step`
enum — not one component per step — so Esc never loses earlier answers.

Step 1 — group:

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › new package                                                    │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  [1 group] › 2 name › 3 source › 4 ignore › 5 profiles › 6 review            │
│                                                                              │
│ ╭ group ─────────────────────────╮ ╭ home ─────────────────────────────────╮ │
│ │▌home                           │ │ Packages here link into ~.            │ │
│ │ config                         │ │                                       │ │
│ │ work                           │ │ source    ~/src/dotfiles/home         │ │
│ │                                │ │ target    ~                           │ │
│ │                                │ │ packages  4: git, shell, ssh, tmux    │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ ↑↓ move  Enter next  Esc cancel                        theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Step 2 — name (typing filters existing packages; the first row is always `+ create`):

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › new package                                                    │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  ✓ group › [2 name] › 3 source › 4 ignore › 5 profiles › 6 review            │
│                                                                              │
│ ╭ name ──────────────────────────╮ ╭ new package ──────────────────────────╮ │
│ │  name  nv█                     │ │ Creates home/nv/ — an empty           │ │
│ │                                │ │ package folder.                       │ │
│ │▌+ create `nv`                  │ │                                       │ │
│ │   existing:                    │ │ path      ~/src/dotfiles/home/nv      │ │
│ │   → nvim  (add files to it)    │ │                                       │ │
│ │                                │ │ Type to filter existing packages;     │ │
│ │                                │ │ pick one to add files to it.          │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ type name  ↑↓ pick  Enter next  Esc back               theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Step 3 — source (radio):

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › new package                                                    │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  ✓ group › ✓ name › [3 source] › 4 ignore › 5 profiles › 6 review            │
│                                                                              │
│ ╭ source ────────────────────────╮ ╭ move live files in ───────────────────╮ │
│ │ ○ empty package                │ │ Pick files that already live in ~.    │ │
│ │▌● move live files in           │ │ andre moves them into the package     │ │
│ │                                │ │ and links them back. Nothing is       │ │
│ │                                │ │ lost; the files keep working.         │ │
│ │                                │ │                                       │ │
│ │                                │ │ Next: a file picker opens at ~.       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ ↑↓ move  Enter next  Esc back                          theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Step 3b — live files (opens at the group target; reuses `FileBrowserWidget` in a ListView;
symlinks are shown but cannot be marked):

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › new package › files                                            │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  ✓ group › ✓ name › [3 source] › 4 ignore › 5 profiles › 6 review            │
│                                                                              │
│ ╭ ~/.config/nvim ────────────────╮ ╭ init.lua ─────────────────────────────╮ │
│ │ ■ / lua                        │ │ kind      file, 2.1 KB                │ │
│ │▌■ • init.lua                   │ │ moves to  home/nvim/.config/nvim/     │ │
│ │ □ • lazy-lock.json             │ │ marked    2 items                     │ │
│ │ □ ~ spell  → ~/sync/spell      │ │                                       │ │
│ │ □ / .cache                     │ │ ~ symlinks cannot be moved in.        │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Space mark  → open  ← up  Enter next  Esc back         theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Step 4 — ignore files (`←→` cycles the template: none / common secrets):

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › new package                                                    │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  ✓ group › ✓ name › ✓ source › [4 ignore] › 5 profiles › 6 review            │
│                                                                              │
│ ╭ ignore files ──────────────────╮ ╭ .gitignore ───────────────────────────╮ │
│ │▌■ .gitignore                   │ │ Keeps secrets out of git. andre       │ │
│ │     template  common secrets ▸ │ │ writes it to the package root.        │ │
│ │ □ .stow-local-ignore           │ │                                       │ │
│ │                                │ │ *.env                                 │ │
│ │                                │ │ .env*                                 │ │
│ │                                │ │ *history*                             │ │
│ │                                │ │ *.key  *.pem                          │ │
│ │                                │ │ secrets/                              │ │
│ │                                │ │ .cache/                               │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Space toggle  ←→ template  Enter next  Esc back        theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Step 5 — profiles (none is valid):

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › new package                                                    │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  ✓ group › ✓ name › ✓ source › ✓ ignore › [5 profiles] › 6 review            │
│                                                                              │
│ ╭ add to profiles ───────────────╮ ╭ personal ─────────────────────────────╮ │
│ │▌■ personal                     │ │ Personal laptop                       │ │
│ │ □ base                         │ │                                       │ │
│ │ □ agent-work                   │ │ packages  7                           │ │
│ │                                │ │ after     8 (+ home/nvim)             │ │
│ │                                │ │                                       │ │
│ │                                │ │ None is fine: the package still       │ │
│ │                                │ │ shows under packages.                 │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Space toggle  Enter next  Esc back                     theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Step 6 — review (wide panel, layout E). Enter creates → execute screen (2.9) with one
row per operation group (moves, links, files, profile edits):

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › new package                                                    │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  ✓ group › ✓ name › ✓ source › ✓ ignore › ✓ profiles › [6 review]            │
│                                                                              │
│ ╭ review · new package home/nvim ──────────────────────────────────────────╮ │
│ │ move   ~/.config/nvim/init.lua → home/nvim/.config/nvim/                 │ │
│ │ move   ~/.config/nvim/lua/     → home/nvim/.config/nvim/                 │ │
│ │ link   ~/.config/nvim/init.lua ⇢ package                                 │ │
│ │ link   ~/.config/nvim/lua      ⇢ package                                 │ │
│ │ create home/nvim/.gitignore  (common secrets, 7 lines)                   │ │
│ │ edit   profiles/personal.yml  + home/nvim                                │ │
│ │                                                                          │ │
│ │ 2 moves · 2 links · 1 file · 1 profile                                   │ │
│ │                                                                          │ │
│ │                                                                          │ │
│ ╰──────────────────────────────────────────────────────────────────────────╯ │
│                                               [ Esc back ]  [ Enter create ] │
│                                                                              │
│──────────────────────────────────────────────────────────────────────────────│
│ Enter create  Esc back                                 theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

### 2.8 Review (stow / restow / unstow packages, profile run, bootstrap)

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › packages › review                                              │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ stow 3 packages ───────────────╮ ╭ home/shell ───────────────────────────╮ │
│ │ ▾ home                         │ │ stow home/shell                       │ │
│ │▌    ◐ shell                    │ │                                       │ │
│ │     → tmux                     │ │ engine    native                      │ │
│ │ ▾ config                       │ │ target    ~                           │ │
│ │     → kitty                    │ │ links     2 new, 2 kept               │ │
│ │                                │ │                                       │ │
│ │                                │ │ $ stow -d ~/src/dotfiles/home         │ │
│ │                                │ │   -t ~ -S shell                       │ │
│ │                                │ │                                       │ │
│ │                                │ │ dry-run is off: files change.         │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│                                                   [ n cancel ]  [ y stow 3 ] │
│──────────────────────────────────────────────────────────────────────────────│
│ y stow  n cancel  ↑↓ inspect                           theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Replaces `ConfirmComponent`'s text dump. Left: tree of what will run (no marks). Right:
the engine-aware preview for the selected package (existing `confirm` preview text),
target, link counts, and a `dry-run is on/off` line (`warn` when off and action is unstow).
Action bar: `[ n cancel ]  [ y <action> N ]`.

### 2.9 Execute (progress → done)

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › packages › run                                                 │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  stow  ████████████████████░░░░░░░░░░░░░░░░░░░░  2/3  ⣼                      │
│ ╭ results ───────────────────────╮ ╭ home/tmux ────────────────────────────╮ │
│ │ ✓ home/shell                   │ │ linked 3 files                        │ │
│ │▌✓ home/tmux                    │ │                                       │ │
│ │ ⣼ config/kitty                 │ │ ⇢ ~/.tmux.conf                        │ │
│ │                                │ │ ⇢ ~/.config/tmux/theme.conf           │ │
│ │                                │ │ ⇢ ~/.local/bin/tmux-session           │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ ↑↓ inspect  running…                                   theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › packages › run                                                 │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  done  ✓ 2 succeeded  ✗ 1 failed                                             │
│ ╭ results ───────────────────────╮ ╭ config/kitty ─────────────────────────╮ │
│ │ ✓ home/shell                   │ │ stow failed                           │ │
│ │ ✓ home/tmux                    │ │                                       │ │
│ │▌✗ config/kitty                 │ │ ✗ existing target is not owned by     │ │
│ │                                │ │   stow: .config/kitty/kitty.conf      │ │
│ │                                │ │                                       │ │
│ │                                │ │ A real file sits where the link       │ │
│ │                                │ │ goes. Move it into the package        │ │
│ │                                │ │ with `new package` → existing         │ │
│ │                                │ │ package, or delete it.                │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│                                                               [ Enter done ] │
│──────────────────────────────────────────────────────────────────────────────│
│ Enter done  ↑↓ inspect                                 theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Chip row = gauge + count + spinner; becomes the summary when finished. The cursor follows
the newest result until the user moves it. Detail shows the full output / error for the
selected result (wrapped, scrolls), plus a fix hint for known errors (conflict). Esc and
Enter work only once finished (Enter = `PopAll` to the main menu, as today).

### 2.10 Status (with profile filter)

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › status                                                         │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  profile [all ▾]          ✓ 6  ◐ 1  → 1  ✗ 1                                 │
│ ╭ packages 9 ────────────────────╮ ╭ config/andre ─────────────────────────╮ │
│ │ ▾ home                         │ │ ✗ conflict — a real file sits         │ │
│ │     ✓ git                      │ │   where a link must go                │ │
│ │     ◐ shell                    │ │                                       │ │
│ │     ✓ ssh                      │ │ target    ~/.config/andre             │ │
│ │     → tmux                     │ │                                       │ │
│ │ ▾ config                       │ │ ✗ andre.yml      real file            │ │
│ │▌    ✗ andre                    │ │ ✓ profiles/      linked               │ │
│ │     ✓ kitty                    │ │                                       │ │
│ │     ✓ nvim                     │ │ Fix: new package → config/andre       │ │
│ │ ▸ work                     ✓ 2 │ │ → move live files in.                 │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ f profile  ←→ fold  r rescan  Esc back                 theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Filtered (entered from a profile with `c`, or `--profile` without `--yolo`):

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › profiles › status                                              │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  profile [agent-work ▾]   ✓ 3  → 1  ✗ 1                                      │
│ ╭ agent-work 5 ──────────────────╮ ╭ config/andre ─────────────────────────╮ │
│ │ ▾ home                         │ │ ✗ conflict — a real file sits         │ │
│ │     ✓ git        via base      │ │   where a link must go                │ │
│ │     ✓ shell      via base      │ │                                       │ │
│ │ ▾ config                       │ │                                       │ │
│ │▌    ✗ andre      via base      │ │                                       │ │
│ │ ▾ work                         │ │                                       │ │
│ │     ✓ aws                      │ │                                       │ │
│ │     → kube                     │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ f profile  ←→ fold  r rescan  Esc back                 theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Filter dropdown (`f` or click the chip):

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › status                                                         │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  profile [all ▾]          ✓ 6  ◐ 1  → 1  ✗ 1                                 │
│ ╭ package╭──────────────────────╮╮ ╭ config/andre ─────────────────────────╮ │
│ │ ▾ home │ all packages         ││ │ ✗ conflict                            │ │
│ │        │▌agent-work           ││ │                                       │ │
│ │        │ base                 ││ │                                       │ │
│ │        │ personal             ││ │                                       │ │
│ │        ╰──────────────────────╯│ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Enter pick  Esc cancel                                 theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

While scanning:

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › status                                                         │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│  profile [all ▾]          scanning 4/9 ⣼                                     │
│ ╭ packages 9 ────────────────────╮ ╭ home/tmux ────────────────────────────╮ │
│ │ ▾ home                         │ │ · not scanned yet                     │ │
│ │     ✓ git                      │ │                                       │ │
│ │     ◐ shell                    │ │ target    ~                           │ │
│ │     ✓ ssh                      │ │                                       │ │
│ │▌    · tmux                     │ │                                       │ │
│ │ ▾ config                       │ │                                       │ │
│ │     · andre                    │ │                                       │ │
│ │     · kitty                    │ │                                       │ │
│ │     · nvim                     │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ f profile  ←→ fold  r rescan  Esc back                 theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

- Chip row: `profile [name ▾]` + status summary (glyph counts in status colours).
- Folded group rows show their worst status + count (`▸ work  ✓ 2`).
- Detail: status headline, target, per-file link states, and a fix line for conflict /
  missing. `r` rescans (invalidates `StatusCache`).

### 2.11 Unstow — group

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › unstow                                                         │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ groups 3 ──────────────────────╮ ╭ home ─────────────────────────────────╮ │
│ │▌home                         ~ │ │ Browse links in this group's          │ │
│ │ config               ~/.config │ │ target and remove the ones you        │ │
│ │ work                         ~ │ │ pick.                                 │ │
│ │                                │ │                                       │ │
│ │                                │ │ source    ~/src/dotfiles/home         │ │
│ │                                │ │ target    ~                           │ │
│ │                                │ │ packages  4                           │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ ↑↓ move  Enter browse  Esc back                        theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

One row per group with its target (as today's target select). New: skipped straight to browse when there is exactly one group.

### 2.12 Unstow — browse

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › unstow › ~                                                     │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ ~  · 2 marked ─────────────────╮ ╭ .bashrc ──────────────────────────────╮ │
│ │ □ / .config                    │ │ link      ~/.bashrc                   │ │
│ │ □ / .local                     │ │ points to src/dotfiles/home/shell/    │ │
│ │▌■ ~ .bashrc                    │ │           .bashrc                     │ │
│ │ ■ ~ .profile                   │ │ package   home/shell                  │ │
│ │ □ ~ .tmux.conf                 │ │                                       │ │
│ │ □ ~ .zshrc                     │ │ Unlinking leaves the package file     │ │
│ │                                │ │ in place; only the link goes.         │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Space mark  → open  ← up  Enter review  Esc back       theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

New vs today: regular files are hidden (only folders and links are listed), and Space marks
several links; Enter reviews the marked links (or the cursor link if none) instead of
unlinking one at a time. The current path becomes the list title (no `Path:` line). Detail:
link, where it points, owning package when resolvable.

### 2.13 Unstow — review

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › unstow › review                                                │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ remove 2 links ────────────────╮ ╭ .bashrc ──────────────────────────────╮ │
│ │▌~ .bashrc                      │ │ remove    ~/.bashrc                   │ │
│ │ ~ .profile                     │ │ points to home/shell/.bashrc          │ │
│ │                                │ │                                       │ │
│ │                                │ │ The source file stays in the          │ │
│ │                                │ │ package. Restow home/shell to         │ │
│ │                                │ │ put the link back.                    │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│                                                 [ n cancel ]  [ y remove 2 ] │
│──────────────────────────────────────────────────────────────────────────────│
│ y remove  n cancel  ↑↓ inspect                         theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Same layout D as 2.8, replaces `UnstowConfirmComponent`'s text page.

### 2.14 Settings

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › settings                                                       │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ settings ──────────────────────╮ ╭ verbose ──────────────────────────────╮ │
│ │ stow options                   █ │ How much stow prints, 0–5.            │ │
│ │   action          stow         █ │                                       │ │
│ │▌  verbose         0            █ │ now       0                           │ │
│ │   dry-run         ✗            █ │ saved     0                           │ │
│ │   no-fold         ✗            █ │ key       v  (anywhere)               │ │
│ │   adopt           ✗            █ │                                       │ │
│ │   dotfiles        ✗            █ │ file      ~/.config/andre/andre.yml   │ │
│ │ appearance                     █ │ paths     relative to ~/.config/      │ │
│ │   theme           default      █ │          andre                        │ │
│ │   icons           unicode      █ │                                       │ │
│ │ config                         █ │                                       │ │
│ │   global ignore   3 patterns   │ │                                       │ │
│ │ groups                    +    │ │                                       │ │
│ │   home            ~            │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Space/Enter change  + group  w save  Esc back          theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › settings                                                       │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ settings · unsaved ● ──────────╮ ╭ home ─────────────────────────────────╮ │
│ │   dotfiles        ✗            │ │ Group `home`                          │ │
│ │ appearance                     │ │                                       │ │
│ │   theme           default      │ │ source    ~/src/dotfiles/home         │ │
│ │   icons           unicode      │ │ target    ~                           │ │
│ │ config                         │ │ ignore    1 pattern                   │ │
│ │   global ignore   3 patterns   █ │ packages  4                           │ │
│ │ groups                    +    █ │ profiles  used by 3                   │ │
│ │▌  home            ~            █ │                                       │ │
│ │   config          ~/.config    █ │                                       │ │
│ │   work            ~            █ │                                       │ │
│ │                                █ │                                       │ │
│ │                                █ │                                       │ │
│ │                                █ │                                       │ │
│ │                                █ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ Enter edit  + add  x remove  w save  Esc back          theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

- Left: one list with section headers (`stow options`, `appearance`, `config`,
  `groups +`) — headers are not selectable (cursor skips them). Values right of the label in
  `value`. Each group is its own row (replaces the `group actions` popup).
- Enter/Space on a value row: flags toggle, verbose cycles, action/theme/icons open a
  picker; `global ignore` opens the ignore editor (2.6, global mode). On a group row Enter
  opens the edit dialog; `+` adds a group (name dialog → source browse → target browse,
  same dialogs), `x` removes (dialog).
- Detail: what the setting does, now vs saved value, its global key, then config file and
  resolve dir (moved here from the two info rows at the top of today's screen).
- `w` saves (title shows ` · unsaved ●` while dirty); Esc when dirty → leave dialog.
  Global keys `t v d n a o s` keep their today-behaviour of editing the pending copy while
  this screen is open (`Transition::Handled`).

Picker (theme previews live while moving; Esc reverts):

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › settings                                                       │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ settings ──────────────────────╮ ╭ theme ────────────────────────────────╮ │
│ │ appearance                     │ │ Colour theme. Previews live.          │ │
│ │▌  theme           nord╭ theme ───────────────────╮                       │ │
│ │                       │ default                  │                       │ │
│ │                       │▌nord                     │                       │ │
│ │                       │ dracula                  │                       │ │
│ │                       │ gruvbox                  │                       │ │
│ │                       │ catppuccin               │                       │ │
│ │                       │ tokyo-night              │                       │ │
│ │                       │ solarized                │                       │ │
│ │                       │ monokai                  │                       │ │
│ │                       │                          │                       │ │
│ │                       │ Enter keep  Esc revert   │                       │ │
│ │                       ╰──────────────────────────╯                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ ↑↓ preview  Enter keep  Esc revert                     theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Group edit / browse / remove dialogs:

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › settings                                                       │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ settings ──────────────────────╮ ╭ home ─────────────────────────────────╮ │
│ │▌  home            ~            │ │ Group `home`                          │ │
│ │                                │ │                                       │ │
│ │         ╭ edit group home ─────────────────────────────────────╮         │ │
│ │         │                                                      │         │ │
│ │         │ name    home                               rename ▸  │         │ │
│ │         │▌source  ~/src/dotfiles/home                browse ▸  │         │ │
│ │         │ target  ~                                  browse ▸  │         │ │
│ │         │ ignore  1 pattern                            edit ▸  │         │ │
│ │         │                                                      │         │ │
│ │         │                       [ Esc cancel ]  [ Enter done ] │         │ │
│ │         ╰──────────────────────────────────────────────────────╯         │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ ↑↓ field  Enter change  Esc cancel                     theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › settings                                                       │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ settings ──────────────────────╮ ╭ home ─────────────────────────────────╮ │
│ │▌  home  ╭ source for home ─────────────────────────────────────╮         │ │
│ │         │ ~/src/dotfiles                                       │         │ │
│ │         │                                                      │         │ │
│ │         │ / ..                                                 │         │ │
│ │         │▌/ home                                               │         │ │
│ │         │ / config                                             │         │ │
│ │         │ / work                                               │         │ │
│ │         │ / .git                                               │         │ │
│ │         │                [ Esc cancel ]  [ Enter choose home ] │         │ │
│ │         │                                                      │         │ │
│ │         ╰──────────────────────────────────────────────────────╯         │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ → open  ← up  Enter choose  Esc cancel                 theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › settings                                                       │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ settings ──────────────────────╮ ╭ work ─────────────────────────────────╮ │
│ │▌  work            ~            │ │ Group `work`                          │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │            ╭ remove group ──────────────────────────────────╮            │ │
│ │            │                                                │            │ │
│ │            │  Remove group `work` from andre.yml?           │            │ │
│ │            │  2 profile refs to work/* are removed.         │            │ │
│ │            │  No files are touched.                         │            │ │
│ │            │                                                │            │ │
│ │            │                       [ n keep ]  [ y remove ] │            │ │
│ │            ╰────────────────────────────────────────────────╯            │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ y remove  n keep                                       theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

### 2.15 Help (topics + content)

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › help                                                           │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ help ──────────────────────────╮ ╭ keys ─────────────────────────────────╮ │
│ │▌keys                           │ │ everywhere                            █ │
│ │ mouse                          │ │  ↑↓ / jk    move                      █ │
│ │ status glyphs                  │ │  Enter      open / run                █ │
│ │ profiles                       │ │  Space      mark                      █ │
│ │ packages                       │ │  Esc        back                      █ │
│ │ new package                    │ │  s          cycle the action          █ │
│ │ ignore files                   │ │  v d n a o  stow options              █ │
│ │ about                          │ │  t          theme                     █ │
│ │                                │ │  q          quit                      █ │
│ │                                │ │                                       █ │
│ │                                │ │ lists                                 │ │
│ │                                │ │  ←→         fold / unfold groups      │ │
│ │                                │ │  + e r x    new, edit, rename, delete │ │
│ │                                │ │  w          save edits                │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ ↑↓ topic  PgUp/PgDn scroll  Esc back                   theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

```text
╭──────────────────────────────────────────────────────────────────────────────╮
│ andre  main › help                                                           │
│──────────────────────────────────────────────────────────────────────────────│
│       stow  verbose ▱▱▱▱▱  │  ✗ dry-run  ✗ no-fold  ✗ adopt  ✗ dotfiles      │
│──────────────────────────────────────────────────────────────────────────────│
│ ╭ help ──────────────────────────╮ ╭ status glyphs ────────────────────────╮ │
│ │ keys                           │ │ ✓  stowed    every file linked        │ │
│ │ mouse                          │ │ →  unstowed  no links yet             │ │
│ │▌status glyphs                  │ │ ◐  partial   some files linked        │ │
│ │ profiles                       │ │ ✗  conflict  real file in the way     │ │
│ │                                │ │ ?  missing   source folder not found  │ │
│ │                                │ │ ∅  empty     package has no files     │ │
│ │                                │ │ ·  unknown   not scanned yet          │ │
│ │                                │ │                                       │ │
│ │                                │ │ ■ □ ▣  marked, unmarked, some         │ │
│ │                                │ │ ● ○    chosen, not chosen             │ │
│ │                                │ │ ▾ ▸    open, folded group             │ │
│ │                                │ │ / ~ •  folder, symlink, file          │ │
│ │                                │ │                                       │ │
│ │                                │ │                                       │ │
│ ╰────────────────────────────────╯ ╰───────────────────────────────────────╯ │
│──────────────────────────────────────────────────────────────────────────────│
│ ↑↓ topic  PgUp/PgDn scroll  Esc back                   theme default  q quit │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Topics: keys, mouse, status glyphs (the old legends), profiles, packages, new package,
ignore files, about (version, config path). Content scrolls with PgUp/PgDn and the wheel.

### 2.16 Too small (shown at 60x18 — whole terminal)

```text
                                                            
                                                            
                                                            
                                                            
                                                            
                                                            
          ╭──────────────────────────────────────╮          
          │          terminal too small          │          
          │                                      │          
          │     needs   80 × 24                  │          
          │     now     60 × 18                  │          
          ╰──────────────────────────────────────╯          
                                                            
                                                            
                                                            
                                                            
                                                            
                                                            
```

The failing dimension's number is drawn in `err`, the passing one in `ok`; the box uses
`border`, the title `text`. Same rendering function as today, new text.

## 3. Mouse

### 3.1 Plumbing

- `main.rs`: `execute!(stdout, EnableMouseCapture)` after entering the alternate screen;
  `DisableMouseCapture` in the restore path **and** the panic hook. Handle
  `Event::Mouse(ev)` → `app.mouse_dispatch(ev)`. Ignore `Moved`, `Drag`, `Up`, right/middle
  buttons. Only `Down(Left)`, `ScrollUp`, `ScrollDown` do anything.
- Trait change (`components/mod.rs`):

```rust
/// Mouse input. `prepare` has already stored this frame's hit rects on the component.
fn handle_mouse(&mut self, _ev: MouseEvent, _ctx: &mut AppContext) -> Transition { Transition::None }
```

- Hit rects are computed in `prepare(area)` (or `update`) and stored on the component
  (`ListView` keeps its inner rect; dialogs/action bars keep `Vec<(Rect, KeyEvent)>`).
  `render` never writes them. Mouse events arrive between frames, so the rects from the last
  prepare are the ones on screen.
- Chrome hit rects are owned by `App`: `ui::render` stores
  `app.chrome_hits: Vec<(Rect, KeyEvent)>` built from the same pure functions that build
  the lines (`top_bar_line` / `footer_line` return spans; add `top_bar_hits` /
  `footer_hits` that walk the same spans and return column ranges + key). Computed before
  painting, from the same inputs.
- `App::mouse_dispatch(ev)` order:
  1. A chrome hit (top bar / footer) → synthesize the key: top-bar chips go through
     `handle_global_key` **only if** the component's `input_mode()` is `Normal`
     (same rule as keys); footer hints go through `component_dispatch(key)`.
  2. Otherwise → `component.handle_mouse(ev, ctx)`, then `process_transition`.
- A shared helper turns most mouse handling into key handling, so behaviour stays in one
  place: `fn click_list(view: &mut ListView, ev, len) -> Option<ListClick>` where
  `ListClick::{Select(i), Activate(i), Mark(i)}`; components map `Activate` to their Enter
  path and `Mark` to their Space path.

### 3.2 Rules

| Target | Click | Wheel |
|--------|-------|-------|
| List row | not selected → select; selected → primary verb (D9) | over the list: cursor ±1 per notch (same as ↑↓) |
| Mark glyph cell (`■□▣`) | toggle mark (and select) — one click | — |
| Fold glyph cell (`▾▸`) | fold/unfold (and select) | — |
| Detail pane | — | scroll the detail ±3 lines |
| Chip (tab, filter) | its key (`Tab`, `f`) | — |
| Step bar step | completed step → jump back; others nothing | — |
| Dialog button / action-bar button | its key | — |
| Inside a dialog's list | same as list rows | cursor ±1 |
| Outside an open dialog | ignored (no accidental cancel) | ignored |
| Top-bar chip | pill → `s`; `verbose` → `v`; flags → `d n a o`; `debug` → nothing | — |
| Footer hint / theme chip / `q quit` | that key | — |
| Breadcrumb, banner, separators | nothing (D11) | — |

Primary verb per list: main menu / targets / profiles / settings rows / help topics →
open (Enter); packages, profile edit, unstow browse, wizard file and profile steps → mark
(Space); review / execute / status rows → select only (nothing more to do).

### 3.3 Per screen

| Screen | Hit rects stored on the component |
|--------|------------------------------------|
| Main menu | `ListView` (rows 1–8, 0 for bootstrap) |
| Profiles | `ListView`; empty-state hint line → `+`; dialogs: field + buttons |
| Profile edit | tree `ListView` + mark/fold cells; form rows (description, include marks) ; click in a pane focuses it |
| Packages | tree `ListView` + mark/fold cells; membership dialog list + buttons |
| Ignore editor | chip tabs; `ListView`; `+ add a pattern` line → `+` |
| Wizard | step bar; per-step `ListView`/radio rows/field; review action bar |
| Review / unstow review | `ListView`; action bar buttons |
| Execute | `ListView`; detail scroll; action bar when done |
| Status | filter chip; dropdown list; tree `ListView` (fold cells) ; detail scroll |
| Unstow group / browse | `ListView` (+ mark cells); `→`-into on click of a selected dir |
| Settings | `ListView` (headers inert); picker list; dialog rows + buttons |
| Help | topics `ListView`; content scroll |

### 3.4 Tests

Add `click(app, x, y)` / `wheel(app, x, y, down)` to `andre/tests/helpers/mod.rs` (build a
`MouseEvent`, call `mouse_dispatch`). Tests render first (so `prepare` stored rects), find
coordinates with the existing `row_containing` / `column_of` helpers, click, and assert
state through `as_any().downcast_ref()`.

## 4. Build plan

Every batch: `make lint` and `make test` exit 0, CHANGELOG `[Unreleased]` entry,
README/REFERENCE in sync, tests for each new behaviour, hermetic test rules from AGENTS.md.
"Render test" means `andre/tests/render.rs` at **80x24** with the unicode icon set unless
stated; also render once at 120x40 and 200x50 without panic. Batches are sized for one
small-context builder (≈ 1–3 modules touched + tests).

### U1 — pattern kit (new, before F3; no visible change)
Build `ui/layout.rs`, `ui/list_panel.rs`, `ui/detail.rs`, `ui/dialog.rs` (+ action bar),
`ui/step_bar.rs`, `ui/text_field.rs`, `ui/chips.rs`, new `Icons` fields, `Hint` data in the
footer. Port the main menu onto `ListView` + `Detail` (same look).
- Unit tests per widget (render into a 76x16 `Buffer`): `list_width(76)==34`,
  `list_width(120)==48`; `ListView::prepare` keeps cursor visible after 30 steps on 40 rows;
  `row_at` maps the first inner row to `offset`; empty state renders message + hint;
  `dialog_rect` centred and clamped to 30..=60; primary button is rightmost and filled
  `accent`; `step_line` marks done/current/next with `ok`/`accent`/`text_dim`;
  `TextField` handles chars/Backspace/←→/Home/End; new glyphs width 1 in both sets;
  footer drops lowest-prio hint whole (no partial word) at 80 cols.
- All existing render tests pass unchanged (main menu identical).

### U2 — tree widget
`ui/tree.rs` (`Tree`, `TreeRow`, `Mark`, `tree_line`).
- Unit tests: fold hides children; header mark is `Some` with 1 of 3 marked; Space on
  header marks all non-inherited; inherited rows refuse toggle; `marked()` order is stable
  (config order).

### F3a — menu 1–8, `s`, profiles list/detail + dialogs
Main menu 8 items (2.1); `s` global action key (+ pill hotkey highlight); profiles screen
(2.3) with new/rename/delete dialogs and Enter → existing confirm flow; `+ e r x c` keys (D5).
- Render: 2.1 rows `1`..`8` with the new labels; profiles list shows `◈ agent-work … 5`;
  detail shows `via base` rows; empty state text `no profiles yet`; dialogs show buttons
  `[ Enter create ]`, `[ y delete ]`.
- Interaction: `s` cycles stow→restow→unstow and is ignored in TextInput; `n` on profiles
  still toggles no-fold; `+` opens the new dialog; invalid name keeps the dialog open;
  `x`+`y` deletes the file in a temp config folder; Enter pushes the review/confirm with the
  resolved packages.

### F3b — profile edit + status profile filter (data)
Profile edit (2.4) on `Tree` + form pane + leave dialog; `w` saves the profile file.
Status gets a `profile: Option<String>` filter (rendering stays as today until F6b);
`c` from profiles opens it filtered.
- Interaction: toggling a package then `w` writes `packages:` in order; Esc with edits shows
  the leave dialog; `n` discards; including a profile that creates a cycle is refused with a
  toast; inherited rows don't toggle.
- Render: title `personal · unsaved ●` after an edit; `Tab` moves the `accent` border.

### F4a — merged packages browser (D3) + membership `p`
Replace GroupSelect/PackageSelect with the tree browser (2.5); detail for group/package;
no legend, no splash (D2, D4); `p` membership dialog writes profile files.
- Render: no `legend` panel title anywhere on the packages screen; group row `▾ ▣ home …
  1/4`; unscanned rows show `·`; footer `Enter stow 1` follows marks and action.
- Interaction: Space on header marks group; `←` on a package selects its group row;
  Enter with no marks runs the cursor package; `p` + Space + Enter updates
  `profiles/*.yml`. Old GroupSelect tests are rewritten, not deleted.

### F4b — ignore editor
Ignore editor component (2.6) for packages (`i`), with tabs and match preview.
- Interaction: `+` adds a line, `x` deletes, `w` writes `.gitignore` / `.stow-local-ignore`
  in the package root; Esc dirty → leave dialog.
- Render: chip row `[.gitignore N]`; `.stow-local-ignore` detail contains `REPLACES`.

### F4c — wizard steps 1–3 (+ file picker)
`NewPackageWizard` with the step bar, group, name (filter + create row), source radio,
live-file picker.
- Render: step bar text `[1 group] › 2 name …`; done steps show `✓`.
- Interaction: typing `nv` filters to `nvim`; Esc on step 2 returns to step 1 with the
  group still selected; symlinks can't be marked.

### F4d — wizard steps 4–6 + execute; remove adopt screens
Ignore templates, profiles, wide review, execute via the creation API (F2). Delete the
adopt components/ui files and their footer entries.
- Interaction (tui_e2e): empty package and live-files package end-to-end in a temp HOME;
  profile file gains the ref; `.gitignore` written with the template.
- Render: review lists `move`, `link`, `create`, `edit` lines and the counts line.

### F5 — headless, bootstrap item, docs (as FEATURES.md) + D8
- Render: with a bootstrap fixture, main menu shows `⌂  set up this machine  0` first and the
  one-row title (no `╔════╗` rows) at 80x24; big banner returns once the item is gone.

### F6a — review + execute redesign
`ConfirmComponent` → layout D (2.8); `ExecuteComponent` → chip gauge + results/detail (2.9);
`UnstowConfirm` → 2.13.
- Render: action bar `[ y stow 3 ]` on the last content row; no `Press 'y'` text anywhere;
  execute detail shows the selected result's full error; done chip `✓ 2 succeeded  ✗ 1 failed`.
- Existing execute gauge/summary/scroll tests updated to the new rows.

### F6b — status redesign
Chip row + tree + detail (2.10), filter dropdown, `r` rescan, no legend.
- Render: chip text `profile [all ▾]`; filtered title `agent-work 5`; dropdown lists
  `all packages` first; `·` while scanning.

### F6c — unstow group + browse
Two-pane group list (2.11) and browse (2.12) with marks and link detail; no legend.
- Render: browse detail has `points to`; no `Path:` line above the panel (path is the
  list title).

### F6d — settings
Sectioned list + detail (2.14), groups as rows, pickers as dialogs with live theme preview,
`w` save + leave dialog, global/group ignore via the F4b editor.
- Interaction: Space on `dry-run` flips pending only; `w` writes `andre.yml`; Esc dirty →
  leave dialog; theme picker Esc restores the previous theme; `+` group flow adds a group.
- Render: no `[Update`/`[All saved]` text; section headers present; cursor skips headers.

### F6e — help, titles, too-small
Help topics two-pane (2.15) incl. the glyph legend; lowercase ` › ` breadcrumb with entity
crumbs and left elision; too-small text (2.16).
- Render: title row `andre  main › help`; help glyph topic lists all 7 statuses; too-small
  at 60x18 shows `needs   80 × 24` and `now     60 × 18`.

### F7a — mouse plumbing + lists + chrome
Capture on/off (+ panic hook), `handle_mouse`, `App::mouse_dispatch`, `chrome_hits`,
`ListView` click/wheel, mark/fold cells; wired on every `ListView` screen.
- Interaction: click a top-bar `dry-run` chip toggles the flag; same click while a text
  field is focused does nothing; click a footer `Esc back` pops; click an unselected row
  selects, click again activates (main menu → pushes the screen); on packages a second
  click marks instead of running; wheel moves the cursor; click outside an open dialog
  changes nothing.

### F7b — mouse on dialogs, bars, tabs, steps, detail scroll
Button/action-bar rects, chip rects, step-bar jump-back, detail wheel scroll, dropdown.
- Interaction: clicking `[ y delete ]` deletes; clicking the `.stow-local-ignore` chip
  switches tabs; clicking `✓ group` in the step bar returns to step 1; wheel over a long
  detail moves its scroll offset; help › mouse topic mentions Shift for text selection.

Order: U1 → U2 → F3a → F3b → F4a → F4b → F4c → F4d → F5 → F6a → F6b → F6c → F6d → F6e →
F7a → F7b. (F2 and F1 are core-only and unaffected.)
