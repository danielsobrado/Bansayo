# models/

Local, user-supplied pretrained ONNX files for the future real-camera path (ADR-009). **Do not commit or redistribute them.** Nothing in the engine loads them yet.

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

## Licence note

RTMPose/MMPose and YOLOX code are Apache-2.0. The licence terms of the datasets behind these pretrained weights have not been reviewed by this project (docs/11 risk R8, ADR-009), so the files are local, user-supplied artefacts that must not be committed or redistributed with the application. Nothing in the engine loads them yet.
