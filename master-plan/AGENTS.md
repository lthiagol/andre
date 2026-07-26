# Master Plan — Agent Instructions

> **Rust readiness:** [docs/concepts/01 - Agent Integration/AGENT-READINESS.md](../docs/concepts/01%20-%20Agent%20Integration/AGENT-READINESS.md) — command matrix.  
> **Repo session start:** [../AGENTS.md](../AGENTS.md) — read meta plan and propose next move.

This project uses **spec-driven development**. The `master-plan/` directory is the
single source of truth for what to build, in what order, and how to verify it.

**You must follow these rules when planning or implementing work in this project.**

---

## 1. Non-Negotiable Rules

1. **Never read or edit files under `master-plan/` directly.** Use the `mp` CLI for
   all reads and writes.
2. **Spec before code.** Do not modify application source code until the relevant
   milestone has `spec_status: ready` (approved).
3. **Two-phase milestones.** Phase 1 is the spec (what/why). Phase 2 is the
   implementation plan (how), created only after spec approval.
4. **Reads emit JSON by default.** Use `mp <command>` — omit `--format json` on reads (redundant since M76).
5. **User-facing output** — summarize JSON or defer to `raul`. `--format human` was removed in v2.
6. **After every write, validate.** `mp validate`
7. **Plan-only mode.** When asked to plan without implementing, stop after `mp`
   writes. Do not touch application code.
8. **Execution mode.** Check `mp execution status`. In `planning` mode, do not implement
   unless the user directs a specific milestone/step. In `autonomous` mode, run the
   `next` loop until blocked — then escalate. Never change spec without pausing.

---

## 1a. Plan zone vs code zone

| Zone | What | Rules |
|------|------|-------|
| **Plan** | `master-plan/` | All reads/writes via `mp`. Never hand-edit plan files. |
| **Code** | Application source (`src/`, `tests/`, configs) | Search, read, implement. Harness tools OK (ripgrep, LSP). |

Use code zone to learn **current behavior** during interviews. Record findings in spec
fields (`context.references`, scenarios, ACs) via `mp milestone create --json @-` — not
by editing plan JSON directly.

See toolkit `docs/BROWNFIELD.md` for greenfield vs brownfield routing.

---

## 1b. Agent output contract (do not drift)

`mp` stdout is **JSON by default** on read commands (M76). Installed skills and
templates must match the repo — run `make install` after pulling doc changes.

| Need | Use | Avoid |
|------|-----|-------|
| Read plan state | `mp status`, `mp show milestone <id>` | `mp … --format json` (redundant) |
| Few fields only | `mp show milestone <id> --fields 'milestone.spec_status,steps[].status'` | `mp show … \| jq` |
| Remediation health | `mp show milestone <id> --summary` | jq count aggregates |
| Validate rollup | `mp validate --summary` | jq on validate output |
| Open findings | `mp reviews finding list <id> --open` | `milestone update --json` with findings |
| Batch resolve | `mp reviews finding resolve <id> --all` | hand-edit finding status in plan JSON |
| Write plan | `mp milestone create --json @-`, `mp step done`, etc. | `sed` / editor on `master-plan/` |
| Debug on-disk JSON | `mp show milestone <id> --format raw` | dumping raw JSON to users |
| Human tables | `raul status`, `raul show <id>` | `--format human` (removed) |

**Loop guard:** if the same `mp` write returns success but the next read shows no
change, stop after 2 attempts — read `--help`, check AGENT-READINESS, then
`mp milestone block` + escalate.

---

## 2. Tooling

| Tool | Location |
|------|----------|
| CLI | `mp` (Master Plan CLI — `~/.agents/master-plan/bin/mp`) |
| Plan directory | `./master-plan/` |
| Spec reference | Project docs or `mp plan show` |

If `mp` is not found, tell the user to install the master-plan toolkit. Do not fall
back to editing plan files by hand.

### Intake routing (which lane?)

```text
Too vague / later?     → idea (P1.6) or conversation note
Small fix / polish?    → track (bugfix / tweak)     ✅ works today
Defer scope formally?  → backlog (P3)
Feature / behavior?    → milestone (full spec)
Prod emergency?        → track bugfix — see docs/EMERGENCY.md
```

---

## 3. Workflows

### 3.0 Project brief (first session after init)

Use when `brief.status = in_progress` or `planning_phase = brief`.

```text
1. mp brief todo2. Ask 1–2 questions per pending topic; user brain-dumps freely
3. mp brief edit T01 --body "..."  (repeat for each topic)
4. mp brief add ...                (optional custom topics)
5. mp brief list     # established context
6. mp brief done
7. → charter interview (§3.1) or milestone planning
```

Optional: `mp interview checklist --checklist-type brief` for suggested question rounds.

Do not start milestone specs until `mp brief done` unless the user explicitly skips the brief.

---

### 3.1a Brownfield change (behavior change on existing code)

Use when changing existing behavior — not a greenfield subsystem.

**Small** → track (`§3.7`). **Large** → milestone with explicit before/after.

**Today (pre-P4):**

```text
1. mp doctor2. Code zone: locate current behavior (files, tests)
3. mp interview checklist --checklist-type milestone4. Spec must state: what exists today, what changes, what stays the same
5. context.references → source paths as evidence
6. mp milestone create --json @-  (change_kind: greenfield is OK until P4)
7. Approve → decompose → implement
```

**P4 (delta milestones):** `change_kind: delta`, `delta.domain`, ADDED/MODIFIED/REMOVED;
`mp specs show <domain>` before create; `mp brownfield scan` optional assist.

---

### 3.1 Plan a new feature or bug (interview mode)

Use when the user asks to plan, groom, or spec work without implementing yet.

```text
1. mp interview checklist --type milestone2. Ask the user 2–4 questions per round (skip topics already answered)
3. Propose defaults from codebase analysis; user confirms or corrects
4. mp milestone create --json @-   (spec fields only — no WPs/steps yet)
5. mp milestone set-spec-status <id> review
6. Summarize the spec in natural language
7. On user approval → mp milestone approve <id>
8. mp validate
9. Stop. Do not write application code.
```

**Interview topics — Phase 1 (spec — lean 2.0 model):**
1. Intent — `intent.outcome`, `problem.description`
2. Scope — `scope.in_scope`, `scope.out_of_scope` (minimum 2 exclusions)
3. Acceptance — `acceptance_criteria` (AC-XX + verification command)
4. Design — `design_decisions` (area/choice/rationale) when trade-offs exist
5. Gaps — `open_questions` (Q-XX; resolve before approval)

**Do not scaffold dropped ceremony fields:** behavior/scenarios, FR-XX/NC-XX
requirements blocks, success_criteria (SSC-XX), interface, context.references,
technical_context, assumptions, risks, follow_ups.

**Interview topics — Phase 2 (implementation plan, after approval):**
1. Decomposition — work packages, steps (files, tests, done-when, rollback)

---

### 3.2 Decompose into implementation plan (phase 2)

Use only after the user approves the spec (`spec_status: ready`).

Triggered by: *"break this into steps"*, *"plan implementation for M03"*, *"decompose M03"*.

```text
1. mp milestone groom <id>2. mp milestone decompose <id>
3. mp plan gaps <id>4. mp milestone wp add <id> ... (work package grouping)
5. mp milestone step add <id> --wp WP1 ... (steps S1, S2, … with files, tests, done-when)
6. mp validate
7. Present the implementation plan to the user for confirmation
```

Do not start coding until the user confirms the implementation plan (or explicitly
says to proceed).

---

### 3.2a Split a step (step too large)

```text
1. mp list steps --milestone <id>2. mp milestone step split <id> <step> --json @-   # e.g. S3 → S3, S3.1, S3.2
3. mp validate
```

---

### 3.2b Challenge a plan (stress-test)

Use when the user wants to review, challenge, or find gaps in a spec or implementation plan.

```text
1. mp show milestone <id>2. mp milestone challenge start <id> --scope plan    # or spec | full
3. mp milestone challenge audit <id>4. mp milestone challenge list <id>5. Discuss findings with user
6. mp milestone challenge resolve <id> F-01 --action update-step --payload ...
7. mp validate
8. mp milestone challenge done <id>
```

---

### 3.3 Execute work

Use when implementing approved, decomposed milestones.

```text
1. mp execution status2. mp next3. mp milestone set-status <id> executing     # first step on this milestone only
4. mp milestone step set-status <id> <step> in-progress   # BEFORE code changes
5. Implement application code (outside master-plan/)
6. mp milestone step done <id> <step> --evidence "..."
7. mp validate
8. Repeat 2–7 until all steps done
9. → §3.3b (verify) — do NOT call complete here
```

**Blocked?** `mp milestone block <id> --reason "..."` → `mp execution pause` → escalate to user.  
**Resume:** `mp milestone unblock <id>`.

### 3.3a Execution handoff (autonomous mode)

When the user says “go execute”, “work through the plan”, or similar:

```text
1. mp execution check2. Present execution_ready milestones and blockers
3. User confirms → mp execution handoff
4. Loop: next → step in-progress → code → step done → validate
5. On ambiguity, validate fail, or new scope → mp execution pause + escalate
```

See toolkit `docs/EXECUTION-MODES.md` and `docs/WALKTHROUGH.md`.

---

### 3.3b Verify (self-step, executor)

After all steps are `done`, the executor **verifies** their own work before handing off
to review. This is not a code review — it is proof that the work is honest.

```text
1. Re-run each step's tests value from the project root; confirm green
2. For each AC: run its verification target, then
   mp milestone criterion pass <id> <ac-id> --evidence "<test-name> exit 0"
3. If an AC cannot be honestly passed:
   mp milestone block <id> --reason "AC-05 blocked: <why>"
   → escalate. Do NOT --force.
4. mp milestone complete <id>   # moves to review-ready (NOT done)
5. mp validate
```

**Evidence is test output, not prose.** Record the test name and exit code
(`cargo test -p mp exit 0`, `crates/x.rs pass`). Never write prose claims like
*"Test X verifies Y"* — that is an assertion, not evidence. If you did not run it,
do not claim it.

**Drift happens.** If the implementation diverged from the spec (a step was skipped,
an AC was relaxed, a test was made source-grep instead of behavioral), record it
explicitly: `mp reviews finding add <id> --severity low --desc "drift: <what>"`
rather than hiding it. A reviewer who finds undeclared drift loses trust in the
whole submission; a declared drift is just a decision they can evaluate.

### 3.3c Independent review (mandatory for milestones)

Autonomous execution is **always** followed by an independent review pass — a
different session/context than the executor — before work is considered shipped.
`done` is reachable **only** through `mp reviews pass`, never through `complete` alone.

> **Risk tiering — which items need external review?**
>
> | Item kind | Flow | External review? |
> |-----------|------|------------------|
> | **Track / backlog tweak** | execute → verify → done | **No** — low blast radius, one-pass |
> | **Milestone** | execute → verify → independent review → done | **Yes** — higher risk earns the loop |
>
> If a track grows complex, promote it (`mp track promote --to-milestone`) and it
> inherits the full flow.

```text
1. mp reviews pending                     # find review-ready milestones
2. mp reviews claim <id>                  # → in-review (different agent than executor)
3. mp execution report <id>               # read claims FIRST
4. Verify claims against diff + tests     # drift hides in reports
5. mp reviews pass <id>                   # → done (terminal, reviewer recorded)
   — OR —
   mp reviews finding add <id> ...        # → remediation; findings attached
6. mp validate
```

**Verify, don't trust.** The execution report and per-AC evidence are *claims*.
Read them first, then confirm against the actual diff and test output. A claim
that says *"test X passes"* is verified by running test X, not by trusting the string.

**Remediation loop:** `mp reviews fail` / `finding add` sets the milestone to
`remediation`. Whoever fixes the findings runs `mp milestone set-status <id>
executing`, addresses each finding, re-verifies (§3.3b), and re-completes — the
milestone re-enters the review queue. A finding is closed with
`mp reviews finding resolve <id> <F-XX> --commit <sha>`.

### 3.3d Execution contract

These rules are permanent — not session scratch notes:

1. **Never complete on red tests.** Step `tests` values are a gate, not a
   suggestion — non-`manual:` values are executed from the project root. Red
   tests block the transition.
2. **`--force` is debt, not a shortcut.** It requires a recorded reason and
   creates visible evidence of the bypass. Prefer `mp milestone block <id>
   --reason "..."` + escalation over forcing. A force-bypassed milestone cannot
   reach `done` until the bypass is resolved or explicitly accepted by a reviewer.
3. **Never hand-edit plan files.** Every status transition, evidence string, and
   finding goes through an `mp` command. Editing `master-plan/*.json` directly is
   a plan-zone violation — `mp validate` will not catch it, but the audit trail
   will show the gap.
4. **Evidence is test output, not prose.** `criterion pass --evidence` records
   what ran and its exit code. *"Test X verifies Y"* is a claim, not evidence.
5. **On unfixable failure: block + report.** `mp milestone block <id> --reason`
   and escalate. Never fake completion, silently defer, or mark a step done that
   was not done.
6. **Review is mandatory for milestones.** `done` is reachable only via
   `mp reviews pass` (an independent pass). Tracks skip this — see the risk-tiering
   table in §3.3c.

> **Enforcement note:** The status transitions above (`executed`, `review-ready`,
> `in-review`, `remediation`) and commands like `mp reviews claim` describe the
> target flow. Until the enforcement layer ships, follow the spirit via today's
> commands: complete enters the review queue, and an independent `reviews pass`
> is required before a milestone is considered shipped.

---

### 3.4 Query and report

Use when the user asks for status, summaries, or what's next.

| User intent | Command |
|-------------|---------|
| Overall status | `mp status` → summarize (or `raul status` for user) |
| All milestones (compact) | `raul milestones` |
| Done / pending / in progress / partial | `mp list milestones --filter <preset>` |
| Needs grooming | `mp list milestones --filter grooming` |
| Pending milestones | `mp list milestones --spec-status ready,interview` |
| What's next? | `mp next` or `mp path` (or `raul next` / `raul path` for user) |
| Full work queue | `raul path` |
| Do M4 before M3 | `mp path pin 04 --before 03` |
| What should we do with M03? | `mp milestone groom 03` |
| Steps for a milestone | `mp list steps --milestone <id>` |
| Park idea for later | `mp idea create ...` |
| Small bugfix | `mp track add bugfix ...` |
| Show one milestone | `raul show <id>` (user) or `mp show milestone <id>` (agent) |
| Archived items | `mp list archived` |
| Full validation | `mp validate` |

> For grooming, challenge, and step filters use the commands in
> [docs/AGENT-READINESS.md](../docs/AGENT-READINESS.md).

---

Use when the user defers a topic **without** asking for a plan or fix:

- “Let’s handle this later”
- “Park this idea”
- “Remind me about the installer approach”

```text
mp idea create --title "App installer design" --body "..." --tags installer
mp validate
```

Do **not** create a milestone, track item, or backlog entry unless the user wants formal deferred scope or actionable work.

**Later:**
```text
mp idea listmp idea promote ID-01 --to-milestone    # or --to-backlog | --to-track bugfix
```

---

### 3.6 Defer scope (backlog)

Use during **grooming** when scope is formally deferred from a milestone or charter:

```text
mp backlog add --desc "..." --priority medium --source planning
mp validate
```

---

### 3.7 Track item (lightweight bugfix/tweak)

Use for **small, independent fixes** that do not need a full milestone spec.

**Choose track when:**
- Effort is hours, not days
- No new feature behavior — correctness, polish, or small improvement
- Work can be done and verified in one pass
- Item does not need scenarios, FR-XX, or design decisions

**Choose milestone when:**
- New feature or significant behavior change
- Multiple work packages or cross-cutting design
- Needs interview, spec approval, and acceptance criteria

```text
1. mp interview checklist --checklist-type track-item --kind bugfix2. Ask 1–3 quick questions if fields missing
3. mp track add bugfix --title "..." --problem "..." --verification "..."
4. mp track start bugfix BF-01              # → in-progress
5. Implement fix
6. mp track done bugfix BF-01 --evidence "..."
7. mp validate
```

**Blocked?** Tell the user; do not call `track done`. **Cancel?** `mp track cancel bugfix BF-01`.

If a track item grows large: `mp track promote bugfix BF-03 --to-milestone`

---

### 3.8 Bootstrap (first time only)

```text
mp init
mp doctor
mp brief todo    # first agent task with user
# ... fill brief (see §3.0) ...
mp brief done
```

If charter is empty after the brief, run a charter interview
(`mp interview checklist --checklist-type charter`) before planning milestones.

Use `mp brief list` as context — do not re-ask what the brief already covers.

---

## 4. Spec Lifecycle Gates

| Gate | Rule |
|------|------|
| No code before ready | `spec_status` must be `ready` before `executing` |
| No open questions at ready | Resolve all `Q-XX` before approval |
| Min 2 out-of-scope items | Required before `review` |
| Min 1 acceptance criterion | Required before `review` |
| No impl plan before ready | WPs/steps only after spec approval |
| All steps done before verify | Every step `done` before §3.3b self-verification |
| Evidence is test output | `criterion pass` evidence records test name + exit, not prose |
| `review-ready` requires honest ACs | All `AC-XX` passed with non-prose evidence (or blocked with reason) |
| `done` requires independent review | Milestones reach `done` only via `mp reviews pass`, never `complete` alone |
| Tracks skip external review | Track items go `start → done` (low blast radius) |

If `mp validate` fails, fix via `mp` commands. Do not patch files manually.

---

## 5. Output Conventions

| Audience | Command | Notes |
|----------|---------|-------|
| Agent reads | `mp <cmd>` | JSON default — **no** `--format json` |
| Field projection | `mp <cmd> --fields 'a.b,c[].d'` | Prefer over jq |
| Health rollup | `mp show milestone <id> --summary` | Prefer over jq counts |
| User display | `raul …` or summarize JSON | |
| Debug source | `--format raw` on `show milestone` / `graph` | Agent-only escape hatch (verbatim on-disk JSON or DOT) |

- **Talking to the user:** summarize JSON in clear prose, or defer to `raul`.
- **Never dump raw plan JSON** unless the user explicitly asks.
- **Never pipe `mp` to jq/python** for plan reads when `--fields` or `--summary` exists.

---

## 6. Triggers

Activate this workflow when the user:

- Mentions master plan, milestone, roadmap, spec, backlog, or ideas
- Mentions “later”, “park this”, “remind me about”, or defers a topic without planning it
- Uses `/mp` or CPD skills (`mp-flow` / `mp-runner` / `mp-coordinator`)
- Asks "what's next?", "plan X", "break down X", or "what's the status?"

---

## 7. References

- **Agent playbook (state updates):** toolkit `docs/AGENT-PLAYBOOK.md`
- Full spec model: toolkit `docs/SPEC.md`
- Command reference: toolkit `docs/concepts/06 - Reference/MP-COMMANDS.md`
- Walkthrough example: toolkit `docs/WALKTHROUGH.md`
- Global CPD skills: `~/.agents/skills/mp-flow/`, `mp-runner/`, `mp-coordinator/`
