# Kestrel — working rules for agents

Browser rocket-landing demo: convexified powered-descent guidance (SOCP) in
Rust → WebAssembly. Read `README.md` and `docs/ROADMAP.md` first.

## Where things live

- `docs/ROADMAP.md` — milestones M0–M3 as checklists. Source of truth for what to do next.
- `docs/ARCHITECTURE.md` — what exists (and what is still _planned_).
- `docs/DECISIONS.md` — numbered decision log. Append, never rewrite.
- `docs/SETUP.md` — toolchain and test policy.
- `learn/` — teaching course; owner must be able to rebuild the project from it.
  **Local only: gitignored, never committed or pushed.** Do not link to it
  from any tracked file.
- `LIMITATIONS.md` — what is not done or not measured.

## Rules

1. **Keep docs current.** After any change, update the docs it affects in the
   same or the next commit: tick ROADMAP boxes, move ARCHITECTURE sections from
   _planned_ to built, log decisions, add glossary terms, write the `learn/`
   chapter for each milestone (concepts, why, hard problems, exercises).
2. **Small commits.** One logical change per commit. Conventional prefixes:
   `feat`, `fix`, `docs`, `test`, `refactor`, `chore`, `perf`.
   Before every commit, check `git status` shows nothing under `learn/` or
   `project-ideas.md` staged.
3. **Tests stay fast.** Whole suite under two minutes. Slow tests are
   `#[ignore]` and never run automatically.
4. **Smallest end-to-end first.** No infrastructure for unreached milestones.
5. **No unmeasured claims.** Numbers go in LIMITATIONS.md with how they were measured.
6. **Simulation only.** Say so wherever it matters.
7. **Only do what the owner asked.** Do not start the next milestone unprompted.
8. MIT license; never commit third-party copyrighted assets.

`project-ideas.md` is the owner's private brief (gitignored, never pushed).
Project 2 is this one.
