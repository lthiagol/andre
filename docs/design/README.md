# Design specs

Specs written while building the current UI and config model. They are implemented;
keep them as the rationale behind decisions, not as a to-do list (open work lives in
`todo.txt`, shipped work in `CHANGELOG.md`).

| File | What it covers |
|------|----------------|
| `FEATURES.md` | Config folder, profiles, ignore files, new-package flow, headless `--profile`, mouse |
| `SCREENS.md` | The screen pattern (list + detail), shared widgets, mockups, mouse rules (decisions D1–D11) |
| `FLAT.md` | Removing groups: one packages folder, one target, flat package names (decisions E1–E14) |

`FLAT.md` overrides the other two wherever they mention groups, `group/package` refs,
folding, the unstow target list or the wizard's group step.
