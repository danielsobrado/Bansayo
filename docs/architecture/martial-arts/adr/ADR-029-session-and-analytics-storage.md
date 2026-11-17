# ADR-029 — SQLite for analytics, MCAP for streams, JSON files for profiles

| | |
|---|---|
| Status | **Accepted** |
| Date | 17 November 2026 |
| Fulfils | The revisit clause of ADR-005 in [docs/12](../../../12-decisions.md) |
| Specified in | [12](../12-data-and-progress.md) |

## Context

ADR-005 records inputs to MCAP with a JSON sidecar and defers SQLite until "sessions need
searching, tagging or cross-session measurement queries". Progress tracking is exactly that:
series of measurements across sessions, filtered by athlete, technique, metric, side and
comparability class.

## Decision

1. **SQLite** (one file, `rusqlite` with bundled SQLite, WAL, foreign keys, `secure_delete`) holds
   sessions, sets, repetitions, measurements, scores, events, assessments, baselines, records,
   content snapshots, tracking configurations and calibration sets. One writer thread; a full
   queue never blocks tracking.
2. **MCAP** holds time-indexed streams in three files per session: `recording.mcap` (inputs,
   existing), `motion.mcap` (solved motion, observables, training events), `video.mcap` (original
   camera frames, optional, off by default).
3. **JSON files** hold athlete profiles, calibration profiles and references.
4. Rows are **authoritative**; files are artefacts with independent retention.
5. Re-analysis writes **new rows** linked by `reanalysis_of`; stored results are never overwritten.
6. Every used definition is stored as a **content snapshot** keyed by hash.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| Keep directory listing + JSON only | Rejected: cross-session queries would scan every session |
| DuckDB / Parquet | Rejected: analytical scale not needed; heavier dependency; row-at-a-time writes during a session |
| A database server (PostgreSQL) | Rejected: a service to install; violates local-first simplicity |
| Embedded key-value store (redb, sled) | Rejected: the queries are relational |
| Event log only, aggregates in memory | Rejected: start-up cost grows with history |
| Everything in SQLite, including streams | Rejected: large blobs; loses MCAP tooling and time index |
| Video as MP4 through an encoder | Rejected: transcoding dependency; per-frame timestamps lost. Export to MP4 is offline and optional |

## Rationale

Each store does what it is good at; both are single files without a server. Keeping inputs lets
every row be regenerated, which is what makes "never overwrite" affordable.

## Advantages

- Progress queries are indexed SQL.
- Per-athlete deletion is a cascade plus directory removal.
- Retention can drop video and inputs while keeping years of analytics.

## Disadvantages

- Two stores can disagree after a crash ([19 §1](../19-risks-and-open-questions.md) R33).
- Schema migrations must be maintained.

## Risks

`rusqlite` bundled build in both the workspace and the Tauri shell (O-12); `motion.mcap` size
(O-16).

## Consequences

The existing in-memory session read is replaced by indexed reads before camera data is recorded
(existing R18). Training sessions cannot start without the store; tracking can.

## Reconsider when

- Multi-machine or multi-user access is required ([26](../26-instructor-and-remote-coaching.md)):
  exchange stays file-based export first.
- Analytics queries exceed interactive latency on years of data.
