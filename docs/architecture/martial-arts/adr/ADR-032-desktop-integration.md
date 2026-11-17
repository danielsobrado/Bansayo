# ADR-032 — Desktop: new workspaces in the existing shell; the UI computes nothing

| | |
|---|---|
| Status | **Accepted** |
| Date | 17 November 2026 |
| Extends | ADR-006, ADR-007 in [docs/12](../../../12-decisions.md) |
| Specified in | [10](../10-desktop-architecture.md) |

## Context

The desktop application is React with an imperative three.js viewport inside a Tauri shell that
hosts or attaches to the engine and talks to it only over HTTP and WebSocket. It has no state,
router or chart library by decision. Training adds dashboards, live sessions, attempt analysis, a
builder, progress charts and a calibration wizard.

## Decision

1. Training screens are **workspaces in the same shell** (Train, Technique, Workouts, Progress,
   Calibration); Studio remains the technical tracking view.
2. The **engine is the source of truth** for session state, content, athletes and progress. The UI
   holds view state and unsaved drafts only.
3. Client data follows three existing patterns: a **replace-only store** for state messages, a
   **mutable holder outside React** for per-frame data (observables), and a **query cache keyed by
   URL** invalidated by `content_rev` and session events.
4. The UI performs **no kinematics, metric formulas, scoring, recognition or alignment**. Clips
   arrive as positions computed by the engine for the viewer's skeleton. Unit conversion and
   formatting are the only arithmetic.
5. Charts are **hand-built SVG primitives**; a measurement is rendered only through
   `MeasurementValue`.
6. Data continues to flow over the network contract, never Tauri IPC.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| Tauri IPC commands for training | Rejected (ADR-006): a path the browser build and VR client could not share |
| A state library plus a data-fetching library | Rejected: the existing hooks cover it; revisit on evidence |
| A chart library | Rejected for now: the charts are lines, bands and markers; consistent with the existing rule |
| react-three-fiber | Rejected (ADR-007) |
| Client-side analysis for responsiveness | Rejected: a second implementation of formulas; results would differ from stored ones |
| A separate training application | Rejected: duplicates shell, connection and viewport |

## Rationale

Keeping every derived value in the engine is what makes desktop and VR agree and stored history
reproducible. Reusing the shell keeps one connection, one viewport and one set of primitives.

## Advantages

- One implementation of every number.
- Browser and native builds stay identical.

## Disadvantages

- More HTTP round trips for review screens.
- Hand-built charts cost effort; primitives missing from the code must be built first.

## Risks

Chart requirements outgrowing hand-built SVG (zoom, brushing, tens of thousands of points).

## Consequences

The Technique workspace later hosts the authoring tools of
[20](../20-technique-authoring.md) on the same components (timeline, traces, compare viewport).

## Reconsider when

A chart needs interaction or density that the primitives cannot deliver within a day of work —
adopt one small canvas chart library for that chart type only.
