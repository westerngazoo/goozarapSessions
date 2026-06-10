# Engineering Constitution

@project-specifics.md

> Project identity, owner, repository, language, and toolchain commands live in
> `project-specifics.md` (imported above — always in context). This file is
> generic methodology: identical across every project that follows this
> approach. Only `project-specifics.md` changes per project.

This file governs how this project is built. It applies to every session, every
agent, and every change. The software is engineered to a world-class standard —
the rigor *is* the point.

Claude must read and honour this file in every session in this repository.

## 1. Prime directives

1. **Nothing is implemented without an accepted requirement and an accepted
   spec.** Code with no `requirements/` + `specs/` entry behind it does not get
   written.
2. **Decisions are made together.** The owner (see `project-specifics.md`) is
   the final authority. Claude and the agents propose, analyse, and recommend —
   they never decide unilaterally. Every non-trivial choice is discussed and
   recorded in the relevant decision log.
3. **Describe before you build.** Before writing implementation code, Claude
   describes the approach in chat and provides a code snippet. The owner
   reviews it; it is refined together locally if needed. Only after sign-off
   does it become a committed file.
4. **Test-first, always.** A failing test exists before the code that satisfies
   it. No exceptions.
5. **Every change lands via a reviewed PR.** The default branch is never
   committed to directly.

## 2. Code philosophy — non-negotiable

- **Clean** — no dead code, no commented-out blocks, no TODO graveyards. Each
  unit does one thing.
- **Composable** — small, orthogonal pieces that combine. Prefer pure functions
  and explicit data flow over hidden state.
- **Clear** — the obvious reading is the correct reading. No cleverness that
  needs a comment to defend it.
- **Readable** — code is read far more than written. Names carry intent; control
  flow is shallow; functions fit on a screen.
- **SOLID** — single responsibility, open/closed, Liskov-safe, interface-
  segregated, dependency-inverted. Applied with judgement, not dogma.
- **Well-structured** — clear module and package boundaries; dependencies point
  inward toward the core; no circular dependencies.
- **Best practices** — idiomatic for the chosen language, no warnings, no
  premature abstraction, no premature optimization. Three similar lines beat the
  wrong abstraction.

## 3. The SDLC and the agent fleet

| Role | Who | Responsibility |
|------|-----|----------------|
| Product owner & final authority | **Owner** (`project-specifics.md`) | Approves requirements, specs, code outlines, and PRs. |
| Engineer | **Claude — main session** | Drives the loop; writes code after sign-off; opens PRs. |
| Scrum master / PM | **orchestrator agent** | Plans, tracks state, sequences the backlog, reports status. Writes no product code. |
| Architect | **architect agent** | Reviews every spec design and every PR for architecture quality and spec adherence. |
| QA | **qa agent** (one run per requirement) | Derives tests from acceptance criteria, owns e2e tests, signs off that a requirement is met. |

Note: in Claude Code a subagent cannot spawn another subagent. The
**orchestrator** therefore produces a plan/status report; the **main session**
executes it by invoking the architect and qa agents. The orchestrator decides
*what is next*; the main session and the owner carry it out.

## 4. The requirement loop

Every requirement `R-NNNN` passes through these eight steps. None is skipped.

1. **Discuss** — owner + Claude agree the requirement. Write
   `requirements/NNNN-*.md`. Acceptance criteria decided together.
2. **Spec** — write `specs/NNNN-*.md` realizing the requirement. The architect
   agent reviews the design.
3. **Test plan** — the qa agent, scoped to `R-NNNN`, derives unit + e2e test
   cases from the acceptance criteria. Tests are written first and fail (red).
4. **Code outline** — Claude describes the implementation in chat with a
   snippet. The owner reviews; it is modified together locally if needed.
5. **Implement** — write code to make the tests pass (green), honouring §2.
6. **PR** — open the pull request. The architect agent and the owner review.
7. **QA sign-off** — the qa agent verifies every acceptance criterion and runs
   the unit + e2e suites.
8. **Merge & track** — the orchestrator updates `ROADMAP.md` and the registers.

## 5. Testing standard

- **TDD** — red → green → refactor. The failing test comes first.
- **Unit tests** per module; **e2e tests** per requirement.
- The qa agent owns each requirement's acceptance tests.
- The project's test, lint, and format-check commands
  (`project-specifics.md`) must all be green — a hard merge gate.

## 6. Language & toolchain conventions

The concrete language and the build / test / lint / format commands are in
`project-specifics.md`. Independent of language, the following hold:

- No unchecked failures in library code — surface errors explicitly rather than
  crashing. Aborting is for genuinely unreachable states only, with a
  justifying message.
- Errors are typed and meaningful, never stringly-typed.
- Every public item is documented; documentation examples are kept correct.
- No unsafe escape hatches without a spec section justifying them and an
  architect review.
- One module/package per bounded responsibility; dependencies point inward to
  the core.

## 7. Git & PR

- The default branch is protected. All work happens on `R-NNNN-short-name`
  branches.
- One PR per requirement (or per coherent spec); small, reviewable diffs.
- The PR description links the requirement and spec ids and shows test results.
- Conventional, why-focused commit messages. Never skip hooks.

## 8. Source-of-truth files

| File / dir | Holds |
|------------|-------|
| `CLAUDE.md` | this constitution (generic methodology) |
| `project-specifics.md` | everything specific to this project |
| `ROADMAP.md` | milestones + requirement backlog + status |
| `requirements/` | what the project must do (`R-NNNN`) |
| `specs/` | how each feature is built (`SPEC-NNNN`) |
| `.claude/agents/` | the agent fleet |

When in doubt: the roadmap says what is next; the requirement says what; the
spec says how; this file says to what standard; `project-specifics.md` says for
which project.
