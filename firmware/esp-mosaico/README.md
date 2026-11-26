# ESP-Mosaico native Bansayo tracker

Experimental ESP32-S31 firmware. Streams the onboard BMI270 directly over Wi-Fi
to the existing Rust `esp32_udp` adapter; SlimeVR is not required.

## Install and flash

Use ESP-IDF 6.2 or later with ESP32-S31 target support. Confirm your board
revision and follow Espressif's USB flashing instructions. Flashing replaces
the factory application; back it up first if you need to restore it.

From this directory, with ESP-IDF activated:

```sh
git clone https://github.com/esp-mosaico/esp-mosaico-bsp.git third_party/esp-mosaico-bsp
git -C third_party/esp-mosaico-bsp checkout dbb7509e1be19c35fd091e6a81ac52826a8c58d6
idf.py --preview set-target esp32s31
idf.py menuconfig
idf.py build
idf.py -p PORT flash monitor
```

Under **Bansayo tracker** enter the 2.4 GHz Wi-Fi SSID/password and PC LAN
IPv4 address. Select 100 or 200 Hz (default 200), port 7710, optionally enable
both BMM150 serial diagnostics. Use a WPA2/WPA3 personal network. Credentials
are in the ignored local sdkconfig and are compiled into the firmware: do not
share binaries or sdkconfig containing real credentials. No over-the-air
provisioning yet. Use USB power for long bench tests.

The board prints its station MAC and sensor number (0). Copy
`../../config/mosaico.example.yaml` to `../../config/local-mosaico.yaml` and
substitute that MAC; match rate and port. Start the engine from repository root:

```sh
cargo run -p bansayo-engine --release -- --config config/local-mosaico.yaml --check
cargo run -p bansayo-engine --release -- --config config/local-mosaico.yaml
```

Allow inbound UDP 7710 on the PC's private network only. The HTTP API can stay
on loopback. The native UDP protocol is unauthenticated, with a configured MAC
allowlist, not a security boundary; never expose it to the internet.

Keep the board still at startup until calibration finishes. In Studio, assign
the device and perform the normal mounting calibration in the reference pose.
The touch screen displays actual read rate, quaternion, raw IMU, RSSI, battery
and local send counters. Recalibrate while stationary; then repeat the engine's
mounting calibration. A successful UDP send is not a backend acknowledgment.
For headless operation disable the diagnostic display in menuconfig.

## Design

- Shared C11 serializer and gravity-aided six-axis quaternion filter in
  `../common`. Sensor frame follows the BMI270's right-handed axes; reference
  is +Y up and yaw is arbitrary at calibration. Gyro is rad/s; specific force
  is m/s² including gravity. Both raw vectors are uncorrected by the filter.
- BMI270 uses ±16 g and ±2000 degrees/s, combined sensor register read and
  actual host-side sample-read time (not BMI270 hardware FIFO timestamps).
- Packet format exactly matches
  `crates/bansayo-engine/src/sources/esp32.rs`: little-endian 64 bytes,
  XYZW quaternion, 32-bit wrapping device time and sequence. No magnetometer
  fields exist in v1. Dual magnetic readings are optional serial diagnostics,
  not AHRS inputs. Yaw drifts; camera heading correction can be tested later.
- Sampling uses a periodic RTOS task, with diagnostics at lower rate and
  measured rate reporting; a blocked I2C or UI operation can cause skipped
  intervals. The current firmware is not a hard real-time or FIFO implementation.
- Calibration stops transmission until stationary bias/tilt is available;
  the backend marks stale input naturally. Sequence increments on successful
  sensor reads, so gaps also include periods when calibration suppresses sends.

## Validation

Run portable tests from repository root (CMake and C compiler required):

```sh
cmake -S firmware/common/tests -B firmware/common/tests/build
cmake --build firmware/common/tests/build
ctest --test-dir firmware/common/tests/build --output-on-failure
cargo test -p bansayo-engine --test esp32_source
```

Hardware acceptance remains pending:

1. Check IMU initialization, MAC, sample rate, display, Wi-Fi reconnect and
   engine device status. Confirm rotation, raw vectors and timing diagnostics.
2. Hold the board on each of six faces; compare expected acceleration sign
   and tilt. Rotate 90 degrees about each axis; verify quaternion convention.
3. Keep still for ten minutes; record heading drift, measured sample rate,
   packet gaps and jitter. Reboot device and check engine recovery.
4. Mount securely on the shin, calibrate mounting, perform slow controlled
   movement, then test faster motions without impacts.
5. Add a calibrated camera and test hybrid mode and optical heading correction.
   Use a rigid optical marker and compare timestamps and orientation.
6. Measure power and latency with USB and battery, display enabled/disabled.

TODO: validate build and hardware on both revisions, add FIFO/data-ready
sampling if rate/jitter warrant it, address measured reboot handling issues,
magnetometer calibration/interference rejection and a versioned raw magnetic
protocol if needed. Add runtime provisioning and acknowledged telemetry later.

Upstream: https://github.com/esp-mosaico/esp-mosaico-bsp (pinned above).
