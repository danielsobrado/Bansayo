//! Bounded, decimated inspection of accepted live observations, separate from solver history.
use super::{
    RuntimeMap,
    diagnostic_stream::{Batch, Stream},
};
use bansayo_contracts::{DeviceId, ObservationData, SensorObservation, SourceEvent};
use serde::Serialize;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
    time::Instant,
};

const CAPACITY: usize = 256;
const SAMPLE_INTERVAL_US: u64 = 20_000;

pub type SharedTrace = Arc<Mutex<Trace>>;

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct Point {
    t_us: u64,
    seq: u64,
    device_time_us: Option<u64>,
    values: Vec<f32>,
}

#[derive(Debug, Default)]
struct Channel {
    points: VecDeque<Point>,
    last_seen: Option<Instant>,
}

#[derive(Debug, Default)]
pub struct Trace {
    channels: BTreeMap<&'static str, Channel>,
    timing: Timing,
    full_rate: Stream,
}

#[derive(Debug, Default)]
struct Timing {
    last: Option<(u64, Instant, u64)>,
    intervals: VecDeque<[f32; 2]>,
    gaps: u64,
}

#[derive(Serialize)]
pub struct TimingSnapshot {
    intervals_ms: Vec<[f32; 2]>,
    sequence_gaps: u64,
}

impl Timing {
    fn record(&mut self, o: &SensorObservation) {
        let now = Instant::now();
        if let Some((t, arrival, seq)) = self.last
            && o.t_us >= t
            && o.seq > seq
        {
            if self.intervals.len() == CAPACITY {
                self.intervals.pop_front();
            }
            self.intervals.push_back([
                (o.t_us - t) as f32 / 1000.0,
                now.duration_since(arrival).as_secs_f32() * 1000.0,
            ]);
            self.gaps = self.gaps.saturating_add(o.seq.saturating_sub(seq + 1));
        }
        self.last = Some((o.t_us, now, o.seq));
    }
}

#[derive(Serialize)]
pub struct ChannelSnapshot {
    age_ms: u64,
    points: Vec<Point>,
}

#[derive(Default, Serialize)]
pub struct Snapshot {
    channels: BTreeMap<&'static str, ChannelSnapshot>,
    timing: Option<TimingSnapshot>,
}

impl Trace {
    fn push(&mut self, kind: &'static str, o: &SensorObservation, values: &[f32]) {
        if kind == "orientation" {
            self.timing.record(o);
        }
        self.push_point(
            kind,
            Point {
                t_us: o.t_us,
                seq: o.seq,
                device_time_us: o.device_time_us,
                values: values.to_vec(),
            },
        );
    }

    fn push_point(&mut self, kind: &'static str, point: Point) {
        self.full_rate.push(kind, &point);
        let c = self.channels.entry(kind).or_default();
        c.last_seen = Some(Instant::now());
        if let Some(last) = c.points.back() {
            if point.t_us < last.t_us {
                c.points.clear();
            } else if point.t_us - last.t_us < SAMPLE_INTERVAL_US {
                return;
            }
        }
        if c.points.len() == CAPACITY {
            c.points.pop_front();
        }
        c.points.push_back(point);
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            timing: self.timing.last.map(|_| TimingSnapshot {
                intervals_ms: self.timing.intervals.iter().copied().collect(),
                sequence_gaps: self.timing.gaps,
            }),
            channels: self
                .channels
                .iter()
                .filter_map(|(&kind, c)| {
                    Some((
                        kind,
                        ChannelSnapshot {
                            age_ms: c.last_seen?.elapsed().as_millis() as u64,
                            points: c.points.iter().cloned().collect(),
                        },
                    ))
                })
                .collect(),
        }
    }
}

pub fn record(runtime: &RuntimeMap, event: &SourceEvent) {
    if let SourceEvent::DeviceLink { device_id, .. } = event {
        if let Ok(mut all) = runtime.write()
            && let Some(device) = all.get_mut(device_id)
        {
            device.inspection = None;
        }
        return;
    }
    let SourceEvent::Observation { observation: o } = event else {
        return;
    };
    if !matches!(
        o.data,
        ObservationData::Orientation { .. }
            | ObservationData::AngularVelocity { .. }
            | ObservationData::Acceleration { .. }
            | ObservationData::Position { .. }
            | ObservationData::RigidPose { .. }
            | ObservationData::Distance { .. }
            | ObservationData::Contact { .. }
    ) {
        return;
    }
    let trace = {
        let Ok(mut all) = runtime.write() else {
            return;
        };
        all.entry(o.device_id.clone())
            .or_default()
            .inspection
            .get_or_insert_default()
            .clone()
    };
    let Ok(mut trace) = trace.lock() else {
        return;
    };
    match &o.data {
        ObservationData::Orientation { q, .. } => trace.push("orientation", o, q),
        ObservationData::AngularVelocity { rad_s, .. } => trace.push("gyro", o, rad_s),
        ObservationData::Acceleration { m_s2, .. } => trace.push("accel", o, m_s2),
        ObservationData::Position { m, .. } => trace.push("position", o, m),
        ObservationData::RigidPose {
            position_m,
            rotation,
            ..
        } => {
            trace.push("pose_rotation", o, rotation);
            trace.push("pose_position", o, position_m);
        }
        ObservationData::Distance { m } => trace.push("distance", o, &[*m]),
        ObservationData::Contact { in_contact, .. } => {
            trace.push("contact", o, &[if *in_contact { 1.0 } else { 0.0 }]);
        }
        _ => {}
    }
}

/// Auxiliary board telemetry is diagnostic-only, never fed into the pose solver.
pub fn magnetic(
    runtime: &RuntimeMap,
    id: &DeviceId,
    kind: &'static str,
    t_us: u64,
    seq: u32,
    device_time_us: u32,
    values: [i16; 3],
) {
    let trace = {
        let Ok(mut all) = runtime.write() else {
            return;
        };
        all.entry(id.clone())
            .or_default()
            .inspection
            .get_or_insert_default()
            .clone()
    };
    if let Ok(mut trace) = trace.lock() {
        if let Some(last) = trace.channels.get(kind).and_then(|c| c.points.back()) {
            let delta = seq.wrapping_sub(last.seq as u32);
            if delta == 0 || delta >= (1 << 31) {
                // A reboot may restart the auxiliary stream. Reject reordered packets
                // unless the channel has been silent long enough to be a new stream.
                if t_us.saturating_sub(last.t_us) < 500_000 {
                    return;
                }
                trace.channels.remove(kind);
            }
        }
        trace.push_point(
            kind,
            Point {
                t_us,
                seq: u64::from(seq),
                device_time_us: Some(u64::from(device_time_us)),
                values: values.map(f32::from).to_vec(),
            },
        );
    }
}

pub fn snapshot(runtime: &RuntimeMap, id: &DeviceId) -> Snapshot {
    let trace = runtime
        .read()
        .ok()
        .and_then(|all| all.get(id)?.inspection.clone());
    trace
        .and_then(|trace| trace.lock().ok().map(|t| t.snapshot()))
        .unwrap_or_default()
}

pub fn batch(runtime: &RuntimeMap, id: &DeviceId, epoch: Option<u64>, after: u64) -> Batch {
    let trace = runtime.write().ok().map(|mut all| {
        all.entry(id.clone())
            .or_default()
            .inspection
            .get_or_insert_default()
            .clone()
    });
    trace
        .and_then(|trace| {
            trace
                .lock()
                .ok()
                .map(|mut t| t.full_rate.read(epoch, after))
        })
        .unwrap_or_default()
}

pub fn clear(runtime: &RuntimeMap) {
    if let Ok(mut all) = runtime.write() {
        for device in all.values_mut() {
            device.inspection = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_is_decimated_bounded_and_cleared() {
        let runtime = RuntimeMap::default();
        let mut observation: SensorObservation = serde_json::from_str(include_str!(
            "../../../../contracts/examples/sensor_observation_orientation.json"
        ))
        .unwrap();
        let id = observation.device_id.clone();
        for i in 0..600 {
            observation.t_us = i * 10_000;
            observation.seq = i;
            record(
                &runtime,
                &SourceEvent::Observation {
                    observation: observation.clone(),
                },
            );
        }
        let result = snapshot(&runtime, &id);
        let points = &result.channels["orientation"].points;
        assert_eq!(points.len(), CAPACITY);
        assert_eq!(points.last().unwrap().seq, 598);
        let timing = result.timing.unwrap();
        assert_eq!(timing.intervals_ms.len(), CAPACITY);
        assert_eq!(timing.sequence_gaps, 0);
        assert!(timing.intervals_ms.iter().all(|p| p[0] == 10.0));
        assert!(
            points
                .windows(2)
                .all(|w| w[1].t_us - w[0].t_us >= SAMPLE_INTERVAL_US)
        );
        record(
            &runtime,
            &SourceEvent::DeviceLink {
                device_id: id.clone(),
                connected: false,
                t_us: 6_000_000,
            },
        );
        assert!(snapshot(&runtime, &id).channels.is_empty());
    }
}
