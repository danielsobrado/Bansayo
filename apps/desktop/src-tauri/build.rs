fn main() {
    println!("cargo:rerun-if-env-changed=VITE_ENGINE_URL");
    tauri_build::build();
}
