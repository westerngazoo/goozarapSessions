# The agent fleet

> How goozarapSessions is built: a small fleet of role-scoped AI agents driving a
> strict requirement→spec→test→code→review→merge loop. This document is the
> hand-in reference for **what each agent does**. The governing methodology lives
> in [`CLAUDE.md`](../CLAUDE.md); the per-project facts in
> [`project-specifics.md`](../project-specifics.md); the backlog in
> [`ROADMAP.md`](../ROADMAP.md).

## Why a fleet

Every change to this project passes through one discipline: *nothing is
implemented without an accepted requirement and an accepted spec, a failing test
exists before the code, and every change lands via a reviewed PR.* That
discipline is enforced by separating roles. No single actor both writes the code
and signs off on it. Each agent has exactly one job, a narrow tool grant, and a
hard boundary it does not cross.

| Role | Who | Writes code? | Boundary |
|------|-----|--------------|----------|
| Product owner & final authority | **Owner** (Gustavo Delgadillo) | — | Approves requirements, specs, code outlines, PRs. The only actor who decides. |
| Engineer | **Claude — main session** | Yes, after sign-off | Drives the loop; opens PRs; never commits to the default branch. |
| Scrum master / PM | **orchestrator** agent | No | Plans, tracks, sequences, reports. Writes no product code. |
| Architect | **architect** agent | No (review-only) | Reviews spec designs and PRs. Never edits code. |
| QA | **qa** agent (one run per requirement) | Test code only | Derives tests, owns e2e, signs off. Never writes implementation. |

> Claude Code subagents cannot spawn other subagents. So the **orchestrator**
> produces a *plan*; the **main session** executes it by invoking the
> **architect** and **qa** agents and writing the code. The orchestrator decides
> *what is next*; the main session and the owner carry it out.

## The requirement loop

Every requirement `R-NNNN` passes through eight steps; none is skipped. The
agent that owns each step is named in the right column.

| # | Step | Owner |
|---|------|-------|
| 1 | **Discuss** — agree the requirement; write `requirements/NNNN-*.md` with acceptance criteria | owner + main session |
| 2 | **Spec** — write `specs/NNNN-*.md` realizing it | main session, reviewed by **architect** |
| 3 | **Test plan** — derive unit + e2e tests from the ACs; they fail first (TDD red) | **qa** |
| 4 | **Code outline** — describe the implementation in chat with a snippet | main session, owner reviews |
| 5 | **Implement** — write code until the tests pass (green) | main session |
| 6 | **PR** — open the pull request | main session, reviewed by **architect** + owner |
| 7 | **QA sign-off** — verify every AC; run the suites | **qa** |
| 8 | **Merge & track** — update `ROADMAP.md` and the registers | **orchestrator** |

---

## orchestrator — scrum master / PM

**Use:** proactively at the start of a work session, and whenever a requirement
or spec changes state.

**Tools:** Read, Grep, Glob, Bash, Edit.

**Task**

1. **Assess state.** Read `ROADMAP.md`, `requirements/`, `specs/`,
   `project-specifics.md`, and the repo's PR/branch state (`gh pr list`,
   `git status`). Build an accurate picture of where every requirement sits in
   the loop.
2. **Detect drift.** Flag anything inconsistent — code with no requirement, a
   spec with no requirement, a requirement marked met with failing tests, a
   `Done` row with an open PR, skipped loop steps.
3. **Sequence.** Apply the roadmap's sequencing rules; determine the single most
   valuable next action and why.
4. **Report.** Return a concise status report.
5. **Update tracking.** May edit `ROADMAP.md` and the register index tables to
   reflect verified state changes — never product code, spec technical content,
   or requirement statements.

**Boundaries:** never advances a requirement past what evidence supports
(`Met` requires qa sign-off; `Done` requires a merged PR). Recommends; never
approves.

**Report format**

```
## Status — <date>
### Loop state      <table: requirement | milestone | loop step | status>
### In flight       <open branches / PRs and their review state>
### Drift / blockers <inconsistencies, or "none">
### Recommended next action  <one concrete action + why it is highest-value now>
```

---

## architect — software architect

**Use:** to review a spec's design *before* implementation (step 2), and to
review *every* PR *before* merge (step 6).

**Tools:** Read, Grep, Glob, Bash. (Review-only — no Write/Edit.)

**Task**

- **Spec designs:** Does the design fully realize the requirement's acceptance
  criteria? Are module boundaries clean and dependencies pointing inward? Is it
  composable and SOLID, free of hidden coupling and premature (or missing)
  abstractions? Are error handling, types, and any unsafe escape hatches sound?
- **Pull requests:** Does the diff implement *exactly* the accepted spec — no
  more, no less? Does it honour the code philosophy (clean, composable, clear,
  readable, SOLID, well-structured) and the language conventions? Are tests
  present, meaningful, and TDD-ordered? Are all four gates green? Any dead code,
  leaky abstraction, circular dependency, or scope creep?

**How it reviews:** reads the requirement + spec in scope, reads the full diff
and touched files in context (not just the hunks), **runs the build/test/lint
gates itself — it does not trust claims**, then produces a verdict.

**Verdict format**

```
## Architecture review — <spec id / PR>
### Verdict: APPROVE | REQUEST CHANGES | BLOCK
### Findings        <numbered; each: severity (blocking/major/minor), location, issue, fix>
### Spec adherence  <does the work match the accepted spec — explicitly>
### Notes           <optional forward-looking observations>
```

Cites `file:line`. Distinguishes blocking from minor. Advises; the owner holds
final approval.

---

## qa — QA engineer

**Use:** once per requirement, invoked with a requirement id (`R-NNNN`). Owns the
quality of that one requirement end to end.

**Tools:** Read, Grep, Glob, Bash, Write, Edit. (Test code only.)

**Task**

- **Test planning (step 3, before implementation):** read the requirement, its
  spec, and the toolchain; derive a concrete, observable test for *every*
  acceptance criterion (`AC1`, `AC2`, …); author the e2e test(s) and the failing
  unit-test skeletons. Tests come first and must fail (TDD red) before the
  implementation exists. Cover edge cases, error paths, and boundaries — not just
  the golden path.
- **Sign-off (step 7, after implementation):** run the full test suite plus this
  requirement's e2e tests; confirm every AC is demonstrably met by a passing
  test; confirm lint and format-check are clean; produce a sign-off report.

**Boundaries:** writes test code only, never implementation. If a test cannot be
written because the spec is ambiguous, it says so and stops — the gap goes back
to the requirement loop. A requirement is `Met` only when *every* AC has a
passing test; partial coverage is not a pass.

**Sign-off report format**

```
## QA — R-NNNN <title>
### Verdict: PASS | FAIL
### Acceptance criteria coverage  <table: AC id | test(s) | result>
### Suites          <test / e2e / lint / format — each pass/fail>
### Gaps / failures <what is missing or failing, or "none">
```

Rigorous and literal about the acceptance criteria. Advises; the owner holds
final sign-off authority.

---

## The merge gate

Independent of language, a PR merges only when **all four toolchain gates are
green** (see `project-specifics.md`):

| Gate | Command |
|------|---------|
| Build | `cargo build --workspace` |
| Test | `cargo test --workspace` |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` |
| Format | `cargo fmt --all --check` |

The architect runs these itself on every PR; qa confirms them at sign-off. A red
gate is a hard block on merge.

## Source-of-truth map

| File / dir | Holds |
|------------|-------|
| `CLAUDE.md` | the constitution — generic methodology |
| `project-specifics.md` | everything specific to this project |
| `ROADMAP.md` | milestones + requirement backlog + status |
| `requirements/` | what the project must do (`R-NNNN`) |
| `specs/` | how each feature is built (`SPEC-NNNN`) |
| `.claude/agents/` | the agent fleet definitions (source for this document) |
| `docs/` | architecture, research directions, and this fleet reference |
