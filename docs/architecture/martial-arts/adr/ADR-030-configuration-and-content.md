# ADR-030 — Versioned YAML content over closed catalogues; no expression language

| | |
|---|---|
| Status | **Accepted** |
| Date | 17 November 2026 |
| Extends | ADR-012 (YAML through `serde_norway`) in [docs/12](../../../12-decisions.md) |
| Specified in | [13](../13-configuration-schemas.md), [05 §5–6](../05-technique-library.md) |

## Context

Techniques, combinations, forms, workouts, programs, assessments, scoring models and cues must be
addable without changing the engine, by instructors as well as developers, and must stay
comparable over years. The brief asks for YAML and warns against an unrestricted rule language.

## Decision

1. **Engine configuration** (what is connected, how it is solved) and **content** (what is trained)
   are separate trees: `config/` and `content/`, plus user content under the data directory.
2. Content is **YAML over closed catalogues implemented in code**: observables, metric kinds,
   sequence node types, progression rule kinds, cue condition forms, methods. Content references
   catalogue entries by id.
3. **No expression language**: no arithmetic, variables, functions, includes or templates; only
   `all`/`any` over predicate atoms and single-level `extends`.
4. Every file has a **schema id with a major version** and every definition an integer
   **`version`**. Stored data references `id@version` and a **snapshot** of each used definition is
   kept, so a session is immune to later edits.
5. **Only the engine writes** content files (atomic write through the API); shipped content is
   read-only at runtime; a user id may not shadow a shipped id.
6. Validation produces **path-qualified problems**; an invalid content file is reported and
   skipped, an invalid engine configuration stops the engine.
7. "Plugin" means a source kind (code), a content file (data) or a carrier model (data). Nothing is
   loaded dynamically.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| Embedded expressions (CEL-like) or scripting (Rhai, Lua) in definitions | Rejected: unbounded behaviour, unit errors, no static validation, security surface, results that cannot be explained from a catalogue |
| Techniques defined in Rust | Rejected: every technique an engine release; instructors excluded |
| Content in the database | Rejected: not diffable or reviewable; shipped content belongs in the repository |
| JSON or TOML | Rejected: YAML is the project's configuration format (ADR-012) and reads better for nested sequences |
| JSON Schema as the validation source | Rejected: cross-file and catalogue checks need code anyway; Rust `serde` structs are the source |
| Multi-level inheritance or mixins | Rejected: a definition must be readable alone; one level covers school variants |
| Directory watching | Rejected: rescans happen at start and after engine writes; fewer race conditions |

## Rationale

A closed catalogue makes content checkable before it runs, lets the builder enumerate options, and
keeps every number explainable. When content cannot express something, that is a signal to add a
tested catalogue entry.

## Advantages

- New techniques, drills and workouts are data.
- Historical results keep their meaning (snapshots).
- Content is reviewable and licensable per file.

## Disadvantages

- Some needs wait for a code change.
- Two version concepts (schema major, definition version) to explain.

## Risks

Instructors find the catalogue too narrow ([19 §1](../19-risks-and-open-questions.md) R34);
`serde_norway` maintenance (O-11).

## Consequences

Engine configuration `schema_version` becomes 2. The illustrative files under `examples/` move to
`config/rigs/` and `content/` with the loader and become its first validation corpus.

## Reconsider when

Three or more independent content needs in one release cycle cannot be met by adding catalogue
entries without distorting them.
