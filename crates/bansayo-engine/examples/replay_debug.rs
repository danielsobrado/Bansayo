use bansayo_engine::regress::TrainingFile;
fn main() {
    let id = std::env::args().nth(1).unwrap();
    let dir = std::path::Path::new("golden/synthetic");
    let sid = bansayo_contracts::SessionId::new(id.as_str()).unwrap();
    let t: TrainingFile = serde_json::from_slice(&std::fs::read(dir.join("sessions").join(&id).join("training.json")).unwrap()).unwrap();
    let lib = bansayo_engine::content_store::ContentStore::library_from_snapshots(&t.snapshots).unwrap();
    let rec = bansayo_engine::recording::reader::load(dir, &sid).unwrap();
    println!("started {} events {}", rec.initial.started_t_us, rec.events.len());
    let out = bansayo_engine::training::reanalyse::run(rec, &t.original, &lib, sid).unwrap();
    for e in out.events.iter().filter(|e| !matches!(e.event_type.as_str(), "phase_changed" | "strike")) {
        println!("{} {} {}", e.t_us, e.event_type, e.data.to_string().chars().take(160).collect::<String>());
    }
}
