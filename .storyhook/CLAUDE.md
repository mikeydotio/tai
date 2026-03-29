# Task Management with Storyhook

This project uses **storyhook** (`story` CLI) for work tracking.

**Important:** The `.storyhook/` directory is version-controlled project data. Do NOT add it to `.gitignore`.

## Session lifecycle

1. Run `story context` at the start of every session to understand project state.
2. Run `story next` to find the highest-priority ready task.
3. Update story status as you work: `story TA-<n> is in-progress`
4. Add progress notes: `story TA-<n> "what changed and why"`
5. Mark complete: `story TA-<n> is done "summary of what was delivered"`
6. Run `story handoff --since 2h` at end of session.

## Planning mode

When creating implementation plans, create a story for each discrete work item, phase, or issue:

```
story new "Phase 1: Set up database schema"
story new "Phase 2: Implement API endpoints"
story new "Phase 3: Add authentication middleware"
```

Define relationships between stories to express dependencies and structure:

```
story TA-1 parent-of TA-2
story TA-2 precedes TA-3
story TA-5 relates-to TA-2
story TA-6 obviates TA-7
```

Set priority on each story so `story next` surfaces the right work:

```
story TA-1 priority critical
story TA-4 priority high
story TA-6 priority medium
```

## During execution

- Before starting a story: `story TA-<n> is in-progress`
- When blocked: `story TA-<n> awaits "reason"`
- When unblocked: `story TA-<n> awaits --clear`
- When done: `story TA-<n> is done "what was delivered"`
- To check what's ready: `story next --count 5`
- To see blocked work: `story list --blocked`
- To see the dependency graph: `story graph`

## Commands

| Action | Command |
|---|---|
| Project overview | `story context` |
| Next ready task | `story next` |
| List open stories | `story list` |
| Show a story | `story TA-<n>` |
| Create a story | `story new "<title>"` |
| Add a comment | `story TA-<n> "comment text"` |
| Set priority | `story TA-<n> priority high` |
| Search | `story search "<query>"` |
| Summary stats | `story summary` |
| Dependency graph | `story graph` |
| Session handoff | `story handoff --since 2h` |
