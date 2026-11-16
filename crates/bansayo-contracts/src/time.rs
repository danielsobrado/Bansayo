//! Session-clock time.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Integer microseconds on the session clock: monotonic since engine start, anchored once to UTC
/// as `session_epoch_utc`. The only clock used for ordering and fusion.
pub type TimeUs = u64;

/// How an observation's `t_us` was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TimeQuality {
    /// Device and engine clocks aligned by a two-way exchange.
    Synchronized,
    /// Mapped from the device clock by an offset estimate.
    Estimated,
    /// No device clock; arrival time, minus a configured latency if known.
    ReceiveTime,
}
