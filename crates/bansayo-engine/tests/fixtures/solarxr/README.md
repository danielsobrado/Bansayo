# SolarXR fixtures

`update_two_devices.bin` and `update_no_identifier.bin` are `MessageBundle`s holding one
`DataFeedUpdate`, built by `generator/` with the **official generated Rust code** of
SlimeVR/SolarXR-Protocol at commit `00c38a6` (30 May 2026). The engine decodes them with its own
bounds-checked reader (`src/sources/slimevr/solarxr.rs`), so the tests check that reader against
an independent encoder rather than against itself.

The generator also verifies the engine's `StartDataFeed` request with the official decoder and
FlatBuffers verifier:

```powershell
$env:BANSAYO_SOLARXR_DUMP = "$env:TEMP\start.bin"; cargo test -p bansayo-engine start_data_feed
cd crates/bansayo-engine/tests/fixtures/solarxr/generator
cargo run -- .. "$env:TEMP\start.bin"
```

Expected last line (checked 19 November 2026):
`StartDataFeed verified: minimum_time_since_last=5 device_data=true info=true status=true rotation=true rotation_reference_adjusted=true tps=true position=false raw_angular_velocity=false`
