#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    sync::Mutex,
    time::Duration,
};
use tauri::Manager;

mod probe;

const ENGINE_ADDRESS: &str = "127.0.0.1:7700";
struct HostedEngine(Mutex<Option<bansayo_engine::EngineHandle>>);

/// A listening port is not enough: only attach to a Bansayo protocol-v1 engine.
fn compatible_engine(address: SocketAddr) -> Result<bool, String> {
    let mut socket = match TcpStream::connect_timeout(&address, Duration::from_millis(300)) {
        Ok(socket) => socket,
        // Windows retries a refused loopback connect for about two seconds, so a closed port
        // times out here instead. If the port is in fact taken, binding reports it.
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::TimedOut
            ) =>
        {
            return Ok(false);
        }
        Err(error) => return Err(format!("Could not inspect local engine: {error}")),
    };
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    socket
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    write!(
        socket,
        "GET /api/v1/state HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|e| e.to_string())?;
    let mut response = String::new();
    socket
        .take(64 * 1024)
        .read_to_string(&mut response)
        .map_err(|e| e.to_string())?;
    probe::parse_engine_response(&response).map(|()| true)
}
fn start_engine(
    app: &tauri::App,
) -> Result<Option<bansayo_engine::EngineHandle>, Box<dyn std::error::Error>> {
    let address: SocketAddr = ENGINE_ADDRESS.parse()?;
    if compatible_engine(address)? {
        return Ok(None);
    }
    let selected_config = std::env::var_os("BANSAYO_CONFIG").map(PathBuf::from);
    let path = selected_config
        .clone()
        .unwrap_or(app.path().resource_dir()?.join("config/sim-full-body.yaml"));
    let config = if selected_config.is_some() {
        bansayo_engine::config::load(path)?
    } else {
        bansayo_engine::config::load_with_data_dir(path, app.path().app_data_dir()?)?
    };
    if config.bind != address {
        return Err("The bundled desktop UI connects to 127.0.0.1:7700. Set api.bind to that address in BANSAYO_CONFIG.".into());
    }
    let engine =
        tauri::async_runtime::block_on(bansayo_engine::start(config)).map_err(|e| e.to_string())?;
    Ok(Some(engine))
}
fn main() {
    let app = tauri::Builder::default()
        .setup(|app| {
            app.manage(HostedEngine(Mutex::new(start_engine(app)?)));
            Ok(())
        })
        .build(tauri::generate_context!());
    let app = match app {
        Ok(app) => app,
        Err(error) => {
            eprintln!("Bansayo desktop could not initialise: {error}");
            std::process::exit(1);
        }
    };
    app.run(|app, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event
            && let Ok(mut hosted) = app.state::<HostedEngine>().0.lock()
            && let Some(engine) = hosted.take()
            && let Err(error) = tauri::async_runtime::block_on(engine.shutdown())
        {
            eprintln!("Could not finalise Bansayo engine: {error}");
        }
    });
}
