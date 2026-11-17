//! `motion.mcap` (martial-arts 12 §7.1): solved motion, observables and training events per
//! session, written off the training runner and read back by time range through the MCAP index.
use bansayo_contracts::*;
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{SyncSender, sync_channel};

pub const FRAME: &str = "/bansayo/frame";
pub const OBSERVABLES: &str = "/bansayo/observables";
pub const TRAINING: &str = "/bansayo/training";
/// Capacity of the training → motion recorder channel (14 §3).
pub const CAPACITY: usize = 2048;

pub enum MotionItem {
    Frame(std::sync::Arc<MotionSample>),
    Observables(u64, Vec<ObservableValue>),
    Training(Box<TrainingEvent>),
    State(Box<TrainingState>),
    Finish(tokio::sync::oneshot::Sender<Result<(), String>>),
}

/// One `motion.mcap` writer thread. `push` never blocks; overflow drops the newest and counts.
pub struct MotionRecorder {
    tx: SyncSender<MotionItem>,
    pub dropped: u64,
    pub path: PathBuf,
}

type FileWriter = mcap::Writer<std::io::BufWriter<std::fs::File>>;

fn channel(
    w: &mut FileWriter,
    channels: &mut BTreeMap<&'static str, u16>,
    topic: &'static str,
) -> Result<u16, mcap::McapError> {
    if let Some(id) = channels.get(topic) {
        return Ok(*id);
    }
    let id = w.add_channel(0, topic, "json", &BTreeMap::new())?;
    channels.insert(topic, id);
    Ok(id)
}

fn write(
    w: &mut FileWriter,
    channels: &mut BTreeMap<&'static str, u16>,
    topic: &'static str,
    t_us: u64,
    v: &impl serde::Serialize,
) -> Result<(), String> {
    let id = channel(w, channels, topic).map_err(|e| e.to_string())?;
    let data = serde_json::to_vec(v).map_err(|e| e.to_string())?;
    w.write_to_known_channel(
        &mcap::records::MessageHeader {
            channel_id: id,
            sequence: 0,
            log_time: t_us * 1000,
            publish_time: t_us * 1000,
        },
        &data,
    )
    .map_err(|e| e.to_string())
}

impl MotionRecorder {
    pub fn start(dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let path = dir.join("motion.mcap");
        let file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        let mut writer = mcap::WriteOptions::new()
            .compression(Some(mcap::Compression::Zstd))
            .chunk_size(Some(256 * 1024))
            .create(std::io::BufWriter::new(file))
            .map_err(|e| e.to_string())?;
        let (tx, rx) = sync_channel::<MotionItem>(CAPACITY);
        std::thread::Builder::new()
            .name("bansayo-motion-recorder".into())
            .spawn(move || {
                let mut channels = BTreeMap::new();
                let mut failed: Option<String> = None;
                for item in rx {
                    let r = match item {
                        MotionItem::Frame(s) => {
                            write(&mut writer, &mut channels, FRAME, s.t_us, &*s)
                        }
                        MotionItem::Observables(t, v) => write(
                            &mut writer,
                            &mut channels,
                            OBSERVABLES,
                            t,
                            &serde_json::json!({ "t_us": t, "values": v }),
                        ),
                        MotionItem::Training(e) => write(
                            &mut writer,
                            &mut channels,
                            TRAINING,
                            e.t_us,
                            &serde_json::json!({ "kind": "event", "event": e }),
                        ),
                        MotionItem::State(s) => {
                            let t = s
                                .current
                                .as_ref()
                                .and_then(|c| c.timer.as_ref())
                                .map_or(0, |x| x.started_t_us + x.elapsed_ms * 1000);
                            write(
                                &mut writer,
                                &mut channels,
                                TRAINING,
                                t,
                                &serde_json::json!({ "kind": "state", "state": s }),
                            )
                        }
                        MotionItem::Finish(reply) => {
                            let r = match failed.take() {
                                Some(e) => Err(e),
                                None => writer.finish().map(|_| ()).map_err(|e| e.to_string()),
                            };
                            let _ = reply.send(r);
                            return;
                        }
                    };
                    if let Err(e) = r
                        && failed.is_none()
                    {
                        failed = Some(e);
                    }
                }
                let _ = writer.finish();
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            tx,
            dropped: 0,
            path,
        })
    }

    pub fn push(&mut self, item: MotionItem) {
        if self.tx.try_send(item).is_err() {
            self.dropped += 1;
        }
    }

    /// Finalises the file on the writer thread; never blocks the caller's thread.
    pub fn finish(self) -> tokio::sync::oneshot::Receiver<Result<(), String>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let sender = self.tx;
        std::thread::spawn(move || {
            let _ = sender.send(MotionItem::Finish(tx));
        });
        rx
    }
}

fn summary(file: &mut std::fs::File) -> Result<mcap::Summary, String> {
    let mut reader = mcap::sans_io::summary_reader::SummaryReader::new();
    while let Some(event) = reader.next_event() {
        match event.map_err(|e| e.to_string())? {
            mcap::sans_io::summary_reader::SummaryReadEvent::ReadRequest(need) => {
                let n = file.read(reader.insert(need)).map_err(|e| e.to_string())?;
                reader.notify_read(n);
            }
            mcap::sans_io::summary_reader::SummaryReadEvent::SeekRequest(to) => {
                reader.notify_seeked(file.seek(to).map_err(|e| e.to_string())?);
            }
        }
    }
    reader
        .finish()
        .ok_or_else(|| "Motion file has no summary.".into())
}

/// Messages of `topic` with log time in `[t0_us, t1_us]`, read through the chunk index: only
/// the chunks overlapping the range are read from disk.
pub fn read_range(
    path: &Path,
    topic: &str,
    t0_us: u64,
    t1_us: u64,
) -> Result<Vec<(u64, Vec<u8>)>, String> {
    let mut file = std::fs::File::open(path)
        .map_err(|_| "Motion was not recorded for this session.".to_owned())?;
    let summary = summary(&mut file)?;
    let options = mcap::sans_io::indexed_reader::IndexedReaderOptions::new()
        .include_topics([topic])
        .starting_at(t0_us * 1000)
        .ending_before(t1_us * 1000 + 1);
    let mut reader =
        mcap::sans_io::indexed_reader::IndexedReader::new_with_options(&summary, options)
            .map_err(|e| e.to_string())?;
    let mut buffer = vec![];
    let mut out = vec![];
    while let Some(event) = reader.next_event() {
        match event.map_err(|e| e.to_string())? {
            mcap::sans_io::indexed_reader::IndexedReadEvent::ReadChunkRequest {
                offset,
                length,
            } => {
                file.seek(SeekFrom::Start(offset))
                    .map_err(|e| e.to_string())?;
                buffer.resize(length, 0);
                file.read_exact(&mut buffer).map_err(|e| e.to_string())?;
                reader
                    .insert_chunk_record_data(offset, &buffer)
                    .map_err(|e| e.to_string())?;
            }
            mcap::sans_io::indexed_reader::IndexedReadEvent::Message { header, data } => {
                out.push((header.log_time / 1000, data.to_vec()));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_reads_return_only_the_requested_window() {
        let dir = tempfile::tempdir().unwrap();
        let mut rec = MotionRecorder::start(dir.path()).unwrap();
        for k in 0..2000u64 {
            rec.push(MotionItem::Observables(k * 10_000, vec![]));
            if k % 2 == 0 {
                rec.push(MotionItem::Frame(std::sync::Arc::new(MotionSample {
                    seq: k,
                    t_us: k * 10_000,
                    origins: vec![],
                    body: None,
                    planar: vec![],
                })));
            }
        }
        let done = rec.finish();
        done.blocking_recv().unwrap().unwrap();
        let path = dir.path().join("motion.mcap");
        let got = read_range(&path, OBSERVABLES, 5_000_000, 5_990_000).unwrap();
        assert_eq!(got.len(), 100);
        assert_eq!(got[0].0, 5_000_000);
        assert_eq!(read_range(&path, FRAME, 0, 100_000).unwrap().len(), 6);
        assert!(read_range(&dir.path().join("none.mcap"), FRAME, 0, 1).is_err());
    }
}
