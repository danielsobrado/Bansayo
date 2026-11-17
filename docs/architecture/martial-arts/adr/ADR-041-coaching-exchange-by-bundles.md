# ADR-041 — Sharing between people is file bundles; no live or hosted service

| | |
|---|---|
| Status | **Proposed** (P2); the export half exists in design ([12 §8](../12-data-and-progress.md)) |
| Date | 17 November 2026 |
| Extends | [ADR-031](ADR-031-local-first-execution.md), [ADR-029](ADR-029-session-and-analytics-storage.md) |
| Specified in | [26](../26-instructor-and-remote-coaching.md) |

## Context

Instructors should be able to give workouts, review a student's attempts and return corrections;
technique authors should be able to publish definitions and references; validators may need to
exchange golden sessions. The platform is local-first with no accounts.

## Decision

1. Everything exchanged between installations is a **bundle**: one zip with a
   `bansayo.bundle/v1` manifest, of kind `content_pack`, `review_request` or `feedback`.
2. A bundle is **self-contained and minimal**: selected repetitions, skeleton-level motion by
   default, the content snapshots and provenance records its measurements refer to, per-file
   hashes, and a consent record stating what is shared, why and with whom.
3. The application **neither uploads nor downloads**. People move the file by their own means.
4. Imported sessions live in a **guest area**: reviewable, never part of a local athlete's
   progress; imported definitions are `draft` locally.
5. Feedback attaches to the original repetitions by `(session, rep_index)`; an attached workout
   appears as a proposal.
6. Live remote sessions, multi-athlete classes, coach dashboards as a service and accounts are
   **out of this architecture**.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| A hosted coaching service | Rejected: accounts, operations, a privacy and compliance burden, and a dependency for ordinary training |
| Peer-to-peer live streaming of sessions | Rejected: NAT traversal and latency engineering for a use case that works asynchronously |
| Share the whole data directory or database | Rejected: over-shares; no consent granularity |
| Video-only sharing through existing apps | Insufficient alone: loses measurements, phases and provenance; video may still be included by consent |
| Signed bundles | Rejected for now: no key distribution to make signatures meaningful; hashes detect corruption |
| A shared-folder sync convention | Deferred: possible later as a convenience over the same bundles |

## Rationale

Asynchronous review of selected attempts is what a coach actually does with recordings, and a file
is the only transport that keeps the no-cloud guarantee simple to state.

## Advantages

- No infrastructure; privacy stays a local decision at each export.
- One container serves coaching, content publishing and dataset exchange.

## Disadvantages

- Manual transfer; no notifications or live interaction.
- Version skew between installations must be handled at import.

## Risks

A sent file cannot be recalled; the export dialog must make contents and consequences plain.

## Consequences

Stable identifiers and versions on everything exportable are a requirement from Phase 1, even
though import is built much later.

## Reconsider when

A school or club asks for many-to-one review with turnaround expectations that manual file
transfer cannot meet.
