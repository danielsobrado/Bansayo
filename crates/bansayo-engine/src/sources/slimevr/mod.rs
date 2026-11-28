//! SlimeVR trackers through SlimeVR Server's SolarXR WebSocket (ADR-008, ADR-022 item 5).
//!
//! What SolarXR gives, verified against the server source (`DataFeedBuilder.kt`): the fused
//! orientation per tracker, tracker status, data ticks per second, battery and RSSI. It does not
//! send raw angular velocity or acceleration, and no sample timestamps: a data feed is a snapshot
//! of the server's latest state, sent at most every `feed_interval_ms`. Observations are therefore
//! stamped on arrival minus `latency_ms` (`time_quality: receive_time`), and a snapshot repeating
//! the previous rotation is not a new measurement, so it is not republished (except as a
//! keep-alive, so a perfectly still tracker does not turn stale).
pub mod solarxr;

use crate::config::SlimevrSpec;
use crate::sources::{Ingest, Source, SourceContext, SourceError};
use bansayo_contracts::*;
use futures_util::{SinkExt, StreamExt};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

/// A still tracker's unchanged rotation is republished this often so it stays fresh.
const KEEP_ALIVE_US: u64 = 100_000;
/// Battery and signal are reported at most this often.
const DIAGNOSTICS_US: u64 = 1_000_000;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const FEED_TIMEOUT: Duration = Duration::from_secs(10);
const BACKOFF_MIN: Duration = Duration::from_millis(500);
const BACKOFF_MAX: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct SlimevrSource {
    pub spec: SlimevrSpec,
    pub runtime: crate::sources::RuntimeMap,
}

/// Device id for a tracker that is not listed in the configuration.
pub fn generated_id(key: &str) -> Option<DeviceId> {
    let clean: String = key
        .chars()
        .filter_map(|c| match c {
            '/' => Some('-'),
            c if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' => Some(c),
            _ => None,
        })
        .collect();
    DeviceId::new(format!("SVR-{clean}")).ok()
}

impl SlimevrSource {
    pub fn descriptor(
        &self,
        id: &DeviceId,
        name: Option<&str>,
        rate: Option<f32>,
    ) -> DeviceDescriptor {
        DeviceDescriptor {
            id: id.clone(),
            source_id: self.spec.id.clone(),
            kind: DeviceKind::ImuTracker,
            origin: Origin::Live,
            model: Some(name.map_or_else(
                || "SlimeVR tracker".into(),
                |n| format!("SlimeVR tracker ({n})"),
            )),
            firmware: None,
            transport: Transport::Websocket,
            capabilities: vec![
                SensorCapability::Orientation,
                SensorCapability::Battery,
                SensorCapability::SignalStrength,
            ],
            nominal_rate_hz: rate,
            camera: None,
        }
    }

    /// Trackers listed in the configuration, announced before any connection.
    pub fn descriptors(&self) -> Vec<DeviceDescriptor> {
        self.spec
            .trackers
            .iter()
            .map(|(id, _)| self.descriptor(id, None, None))
            .collect()
    }
}

/// Per-connection bookkeeping that survives reconnects.
#[derive(Default)]
struct State {
    seq: BTreeMap<(DeviceId, bool), u64>,
    last: BTreeMap<DeviceId, ([u32; 4], u64)>,
    diagnostics: BTreeMap<DeviceId, u64>,
    announced: BTreeSet<DeviceId>,
    linked: BTreeSet<DeviceId>,
    ignored: BTreeSet<String>,
    invalid: u64,
}

fn send(context: &SourceContext, event: SourceEvent) -> bool {
    context.events.try_send(Ingest::Event(event)).is_ok()
}

impl SlimevrSource {
    fn id_for(
        &self,
        t: &solarxr::Tracker,
        state: &mut State,
        context: &SourceContext,
    ) -> Option<DeviceId> {
        if let Some((id, _)) = self.spec.trackers.iter().find(|(_, key)| key == &t.key) {
            return Some(id.clone());
        }
        if !self.spec.announce_unlisted || !t.stable {
            if state.ignored.insert(t.key.clone()) {
                let why = if t.stable {
                    "not listed under trackers and announce_unlisted is off"
                } else {
                    "the server sends no hardware identifier, so it has no stable identity"
                };
                tracing::info!(source=%self.spec.id, tracker=%t.key, why, "SlimeVR tracker ignored");
            }
            return None;
        }
        let id = generated_id(&t.key)?;
        if !state.announced.contains(&id) {
            let rate = t.poll_rate_hz.filter(|r| r.is_finite() && *r > 0.0);
            if !send(
                context,
                SourceEvent::DeviceAnnounced {
                    descriptor: self.descriptor(&id, t.name.as_deref(), rate),
                },
            ) {
                return None;
            }
            state.announced.insert(id.clone());
            tracing::info!(source=%self.spec.id, tracker=%t.key, device=%id, "SlimeVR tracker announced");
        }
        Some(id)
    }

    fn handle(&self, t: &solarxr::Tracker, now: u64, state: &mut State, context: &SourceContext) {
        let Some(id) = self.id_for(t, state, context) else {
            return;
        };
        let usable = matches!(
            t.status,
            Some(solarxr::STATUS_OK | solarxr::STATUS_BUSY | solarxr::STATUS_OCCLUDED)
        );
        if !usable {
            if state.linked.contains(&id)
                && send(
                    context,
                    SourceEvent::DeviceLink {
                        device_id: id.clone(),
                        connected: false,
                        t_us: now,
                    },
                )
            {
                state.linked.remove(&id);
                state.last.remove(&id);
            }
            return;
        }
        if !state.linked.contains(&id) {
            if !send(
                context,
                SourceEvent::DeviceLink {
                    device_id: id.clone(),
                    connected: true,
                    t_us: now,
                },
            ) {
                return;
            }
            state.linked.insert(id.clone());
            if let Ok(mut r) = self.runtime.write() {
                r.entry(id.clone()).or_default().time_sync = Some(TimeSyncStatus {
                    time_quality: TimeQuality::ReceiveTime,
                    offset_us: None,
                    uncertainty_us: None,
                    drift_ppm: None,
                    settled: false,
                    resets: 0,
                });
            }
        }
        let t_us = now.saturating_sub(self.spec.latency_us);
        if state
            .diagnostics
            .get(&id)
            .is_none_or(|last| now.saturating_sub(*last) >= DIAGNOSTICS_US)
            && (t.battery_pct.is_some() || t.rssi_dbm.is_some())
        {
            state.diagnostics.insert(id.clone(), now);
            self.emit(
                &id,
                t_us,
                None,
                ObservationData::Diagnostics {
                    battery_level: t.battery_pct.map(|p| (p as f32 / 100.0).clamp(0.0, 1.0)),
                    charging: None,
                    rssi_dbm: t.rssi_dbm,
                    quality: None,
                },
                state,
                context,
            );
        }
        let raw = match self.spec.rotation {
            solarxr::RotationField::Raw => t.rotation,
            solarxr::RotationField::ReferenceAdjusted => t.rotation_reference_adjusted,
        };
        let Some(q) = raw.and_then(solarxr::canonical) else {
            if raw.is_some() {
                state.invalid += 1;
                tracing::debug!(device=%id, invalid=state.invalid, "SlimeVR rotation rejected");
            }
            return;
        };
        let bits = q.map(f32::to_bits);
        if let Some((last, at)) = state.last.get(&id)
            && *last == bits
            && now.saturating_sub(*at) < KEEP_ALIVE_US
        {
            return;
        }
        state.last.insert(id.clone(), (bits, now));
        // Occluded or busy: the server still sends its latest rotation; it counts as degraded.
        let confidence = if t.status == Some(solarxr::STATUS_OK) {
            1.0
        } else {
            0.4
        };
        let frame = FrameRef::SensorReference { id: id.clone() };
        self.emit(
            &id,
            t_us,
            Some(confidence),
            ObservationData::Orientation { frame, q },
            state,
            context,
        );
    }

    fn emit(
        &self,
        id: &DeviceId,
        t_us: u64,
        confidence: Option<f32>,
        data: ObservationData,
        state: &mut State,
        context: &SourceContext,
    ) {
        let key = id.clone();
        // Orientation and diagnostics are separate streams with their own sequence numbers.
        let seq_key = (key.clone(), matches!(data, ObservationData::Diagnostics { .. }));
        let seq = state.seq.entry(seq_key).or_insert(0);
        *seq += 1;
        let observation = SensorObservation {
            device_id: key,
            source_id: self.spec.id.clone(),
            origin: Origin::Live,
            seq: *seq,
            t_us,
            device_time_us: None,
            time_quality: TimeQuality::ReceiveTime,
            calibration_ref: None,
            confidence,
            data,
        };
        if context
            .events
            .try_send(Ingest::Event(SourceEvent::Observation { observation }))
            .is_err()
            && let Some(c) = context.dropped.get(id)
        {
            c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// One connection: request the feed, then decode until the server goes away.
    async fn session(&self, context: &SourceContext, state: &mut State) -> Result<(), String> {
        let (mut ws, _) = tokio::time::timeout(
            CONNECT_TIMEOUT,
            tokio_tungstenite::connect_async(self.spec.endpoint.as_str()),
        )
        .await
        .map_err(|_| "connection timed out".to_string())?
        .map_err(|e| e.to_string())?;
        tracing::info!(source=%self.spec.id, endpoint=%self.spec.endpoint, "Connected to SlimeVR Server");
        tokio::time::timeout(
            CONNECT_TIMEOUT,
            ws.send(Message::Binary(
                solarxr::start_data_feed(self.spec.feed_interval_ms).into(),
            )),
        )
        .await
        .map_err(|_| "feed request timed out".to_string())?
        .map_err(|e| e.to_string())?;
        loop {
            let message = tokio::select! {
                _ = context.cancel.cancelled() => return Ok(()),
                m = tokio::time::timeout(FEED_TIMEOUT, ws.next()) => {
                    m.map_err(|_| "SlimeVR feed timed out".to_string())?
                },
            };
            match message {
                Some(Ok(Message::Binary(bytes))) => {
                    let now = context.clock.now_us();
                    match solarxr::decode(&bytes) {
                        Ok(trackers) => {
                            for t in &trackers {
                                self.handle(t, now, state, context);
                            }
                        }
                        Err(e) => {
                            tracing::debug!(source=%self.spec.id, error=%e, "SolarXR message skipped")
                        }
                    }
                }
                Some(Ok(Message::Close(_))) | None => {
                    return Err("the server closed the connection".into());
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(e.to_string()),
            }
        }
    }
}

impl Source for SlimevrSource {
    type Context = SourceContext;
    /// Never fails for an absent or restarting server: it reconnects with bounded backoff until
    /// cancelled, so supervision's restart cap is not spent on a closed SlimeVR window.
    async fn run(self, context: SourceContext) -> Result<(), SourceError> {
        let mut state = State::default();
        let mut backoff = BACKOFF_MIN;
        let mut last_error = String::new();
        loop {
            let started = std::time::Instant::now();
            let result = self.session(&context, &mut state).await;
            if context.cancel.is_cancelled() {
                return Ok(());
            }
            let now = context.clock.now_us();
            for id in std::mem::take(&mut state.linked) {
                let event = Ingest::Event(SourceEvent::DeviceLink {
                    device_id: id,
                    connected: false,
                    t_us: now,
                });
                tokio::select! {
                    _ = context.cancel.cancelled() => return Ok(()),
                    result = context.events.send(event) => {
                        if result.is_err() {
                            return Ok(());
                        }
                    }
                }
            }
            state.last.clear();
            state.diagnostics.clear();
            if let Err(e) = result
                && e != last_error
            {
                tracing::warn!(source=%self.spec.id, endpoint=%self.spec.endpoint, error=%e, "SlimeVR Server unavailable; retrying");
                let _ = context.events.try_send(Ingest::Notice {
                    code: "slimevr_unavailable".into(),
                    message: format!("SlimeVR Server at {} is not reachable ({e}). Start SlimeVR Server; Bansayo reconnects automatically.", self.spec.endpoint),
                });
                last_error = e;
            }
            if started.elapsed() > BACKOFF_MAX {
                backoff = BACKOFF_MIN;
            }
            tokio::select! {
                _ = context.cancel.cancelled() => return Ok(()),
                _ = tokio::time::sleep(backoff) => {}
            }
            backoff = (backoff * 2).min(BACKOFF_MAX);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_are_valid_device_ids() {
        assert_eq!(
            generated_id("AA:BB:CC:DD:EE:01/0").unwrap().as_str(),
            "SVR-AABBCCDDEE01-0"
        );
        assert!(generated_id("weird name/1").is_some());
    }
}
