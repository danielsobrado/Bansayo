# models/

Local, user-supplied pretrained ONNX files and ONNX Runtime for the real-camera path (ADR-009). **Do not commit or redistribute them.** A `kind: camera` source loads them from its `models_dir` (default `./models`); configuration validation reports missing model files.

**Install with `python scripts/setup.py models onnxruntime`** from the repository root: it
downloads the archives below, checks their SHA-256 and extracts exactly the files listed here.
The tables record the sources the script pins.

| File | Source URL | Bytes | SHA-256 | Downloaded |
|---|---|---|---|---|
| rtmpose-m.onnx | https://download.openmmlab.com/mmpose/v1/projects/rtmposev1/onnx_sdk/rtmpose-m_simcc-body7_pt-body7_420e-256x192-e48f03d0_20230504.zip (member `end2end.onnx`) | 54330655 | 5c0a4bf67953e6d2ac43ce15e77dc9d5d354ae18430a47d2c5963a7bc5683e3c | 16 November 2026 |
| yolox-m-humanart.onnx | https://download.openmmlab.com/mmpose/v1/projects/rtmposev1/onnx_sdk/yolox_m_8xb8-300e_humanart-c2c7a14a.zip (member `end2end.onnx`) | 101400344 | 3dea6513388889f0fff4b77bf7a26013600321b9eb9ceb0e9a400a82572f5f23 | 16 November 2026 |

Source zips: rtmpose 50799818 bytes, sha256 f7fbb6c5c11a1bb70f3d445e4ddec5d144ea89ad5649c081ef976f9b24a0b741; yolox 94223081 bytes, sha256 a000224fd8ba283202bc62d4a5fcdfe353adb9f468777dbac1ea2ada2093adde. Zips were deleted after extraction.
The `*.deploy.json`, `*.pipeline.json`, `*.detail.json` files are the archives' metadata, copied verbatim.

## Input/output facts

Read from the ONNX graphs with the `onnx` Python package (graph metadata only, models not run):
- rtmpose-m: input `input` float32 [batch,3,256,192]; outputs `simcc_x` [batch,K,384], `simcc_y` [batch,K,512] (float32; K symbolic in the graph).
- yolox-m-humanart: input `input` float32 [1,3,640,640]; outputs `dets` float32 [1,N,5] and `labels` int64 [1,N] (NMS is inside the graph).

Read from the metadata JSONs (not independently tested):
- rtmpose-m preprocessing: bbox centre/scale with padding 1.25, affine to 192x256 (WxH), RGB, mean [123.675,116.28,103.53], std [58.395,57.12,57.375]. Decode: SimCC, simcc_split_ratio 2.0, no flip test, no DARK.
- yolox-m preprocessing: keep-ratio resize to 640x640, pad to square with 114, BGR (to_rgb false), mean 0, std 1, no scaling to 0..1; post-processing score_thr 0.01 (pipeline) / in-graph NMS iou 0.5.

## ONNX Runtime

The engine loads ONNX Runtime at run time (`ort` with `load-dynamic`, C API 24), so nothing is
linked or downloaded at build time. It looks for `models/onnxruntime/onnxruntime.dll`
(`libonnxruntime.so` / `.dylib` elsewhere), then `ORT_DYLIB_PATH`, then the system search path.
On Windows the DirectML build runs on any DirectX 12 GPU:

| File | Source | Version |
|---|---|---|
| onnxruntime/onnxruntime.dll, DirectML.dll, onnxruntime_providers_shared.dll | PyPI wheel `onnxruntime_directml-1.24.4-cp312-cp312-win_amd64.whl` (members under `onnxruntime/capi/`), sha256 f2ecb68b7b7b259d2ef3112ae760149f9b5a1e7c0fbb73d539da6250a648a614 | 1.24.4 (MIT) |

Linux x64 and macOS arm64 get the CPU build from the official release archives
(`onnxruntime-linux-x64-1.24.4.tgz`, sha256 3a211fbea252c1e66290658f1b735b772056149f28321e71c308942cdb54b747;
`onnxruntime-osx-arm64-1.24.4.tgz`, sha256 93787795f47e1eee369182e43ed51b9e5da0878ab0346aecf4258979b8bba989),
installed as `libonnxruntime.so` / `libonnxruntime.dylib`. Other platforms: set `ORT_DYLIB_PATH`
to an ONNX Runtime 1.24 library.

## Verified output

`rtmpose-m.onnx` outputs **COCO-17** keypoints (K = 17): no heel or toe points, so feet stay
inferred in the camera-only skeleton. The Rust pipeline matched an onnxruntime/OpenCV reference
of the mmdeploy pipeline within 0.01 px on every keypoint of a COCO photo (17 November 2026).
YOLOX labels contain only class 0 (person). RTMPose SimCC scores can exceed 1 and are clamped.
On an RTX 4080 with DirectML, a tracked frame (pose every frame, person detection every tenth)
took 3.7 ms in a release build, 17 ms in a debug build and 74 ms on CPU (debug): one shared
inference thread covers four cameras at 30 Hz in release.

## Licence note

RTMPose/MMPose and YOLOX code are Apache-2.0. The licence terms of the datasets behind these pretrained weights have not been reviewed by this project (docs/11 risk R8, ADR-009), so the files are local, user-supplied artefacts that must not be committed or redistributed with the application.
