#!/usr/bin/env python3
"""Camera calibration for Bansayo with a printed ChArUco board (OpenCV >= 4.7, numpy).

  board    write a printable board image
  capture  save synchronized frame sets from system cameras (stop the engine first)
  solve    intrinsics per camera, extrinsics from one shared board view, then import into the
           engine (POST /api/v1/calibration/profiles) or write the profiles to a JSON file
  selftest render a synthetic multi-camera scene and check that `solve` recovers it

World frame (the engine's): the board lies flat on the floor and defines it. Origin at the
board's first inner corner (raised by --origin-height-m if the board is not on the floor), +Y up
(towards the cameras), +X along the board's first row, +Z = X x Y. Camera frame: +X right,
+Y down, +Z forward, as OpenCV. Distortion is OpenCV's [k1, k2, p1, p2, k3] (Brown-Conrady).

Typical session, two cameras at indices 0 and 1 named as in the engine configuration:
  python scripts/calibrate_cameras.py board --out board.png        # print it flat, measure squares
  python scripts/calibrate_cameras.py capture --camera CAM-FRONT=0 --camera CAM-SIDE=1 --out calib
     (SPACE saves a set: 15-30 sets with the board tilted all over each view, then a few with the
      board flat on the floor at the training spot, seen by every camera; Q quits)
  python scripts/calibrate_cameras.py solve --dir calib --square-m 0.04 --engine http://127.0.0.1:7700
"""
import argparse
import datetime
import json
import math
import sys
import urllib.request
from pathlib import Path

import cv2
import numpy as np

DICTIONARY = cv2.aruco.DICT_5X5_100
# Accept a camera's intrinsics up to this RMS reprojection error, pixels.
MAX_INTRINSIC_RMS_PX = 1.0
MAX_EXTRINSIC_RMS_PX = 2.0
MIN_VIEWS = 8
MIN_CORNERS = 6


def make_board(cols, rows, square_m, marker_m):
    d = cv2.aruco.getPredefinedDictionary(DICTIONARY)
    return cv2.aruco.CharucoBoard((cols, rows), square_m, marker_m, d)


def detect(board, image):
    gray = cv2.cvtColor(image, cv2.COLOR_BGR2GRAY) if image.ndim == 3 else image
    corners, ids, _, _ = cv2.aruco.CharucoDetector(board).detectBoard(gray)
    if ids is None or len(ids) < MIN_CORNERS:
        return None
    obj, img = board.matchImagePoints(corners, ids)
    return obj.reshape(-1, 3).astype(np.float32), img.reshape(-1, 2).astype(np.float32)


def cmd_board(a):
    board = make_board(a.cols, a.rows, 1.0, a.marker_ratio)
    px = a.square_px
    image = board.generateImage((a.cols * px, a.rows * px), marginSize=px // 2)
    cv2.imwrite(a.out, image)
    print(f"Wrote {a.out}: {a.cols}x{a.rows} squares, DICT_5X5_100. Print it flat without scaling, "
          f"measure one square edge in metres and pass it as --square-m to solve.")


def cmd_capture(a):
    cams = []
    for spec in a.camera:
        name, index = spec.split("=")
        cap = cv2.VideoCapture(int(index), cv2.CAP_MSMF if sys.platform == "win32" else cv2.CAP_ANY)
        cap.set(cv2.CAP_PROP_FOURCC, cv2.VideoWriter_fourcc(*"MJPG"))
        cap.set(cv2.CAP_PROP_FRAME_WIDTH, a.width)
        cap.set(cv2.CAP_PROP_FRAME_HEIGHT, a.height)
        if not cap.isOpened():
            sys.exit(f"Camera {index} ({name}) could not be opened. Is the engine still running?")
        (Path(a.out) / name).mkdir(parents=True, exist_ok=True)
        cams.append((name, cap))
    board = make_board(a.cols, a.rows, 1.0, a.marker_ratio)
    saved = len(list((Path(a.out) / cams[0][0]).glob("*.png")))
    print("SPACE saves a set, Q quits.")
    while True:
        frames = []
        for name, cap in cams:
            ok, frame = cap.read()
            if not ok:
                sys.exit(f"{name}: no frame")
            frames.append(frame)
        tiles = []
        for (name, _), frame in zip(cams, frames):
            view = frame.copy()
            found = detect(board, frame)
            colour = (0, 200, 0) if found else (0, 0, 255)
            if found:
                for x, y in found[1]:
                    cv2.circle(view, (int(x), int(y)), 4, colour, -1)
            cv2.putText(view, f"{name} corners: {0 if not found else len(found[1])}", (20, 40),
                        cv2.FONT_HERSHEY_SIMPLEX, 1.0, colour, 2)
            tiles.append(cv2.resize(view, (640, int(640 * view.shape[0] / view.shape[1]))))
        cv2.imshow("bansayo calibration capture", np.hstack(tiles) if len(tiles) < 3 else
                   np.vstack([np.hstack(tiles[i:i + 2] + ([np.zeros_like(tiles[0])] if len(tiles[i:i + 2]) == 1 else []))
                              for i in range(0, len(tiles), 2)]))
        key = cv2.waitKey(1) & 0xFF
        if key == ord("q"):
            break
        if key == ord(" "):
            for (name, _), frame in zip(cams, frames):
                cv2.imwrite(str(Path(a.out) / name / f"{saved:04d}.png"), frame)
            saved += 1
            print(f"saved set {saved}")
    for _, cap in cams:
        cap.release()
    cv2.destroyAllWindows()


def rot_to_quat(r):
    """xyzw unit quaternion of a rotation matrix."""
    w = math.sqrt(max(0.0, 1.0 + r[0, 0] + r[1, 1] + r[2, 2])) / 2.0
    x = math.copysign(math.sqrt(max(0.0, 1.0 + r[0, 0] - r[1, 1] - r[2, 2])) / 2.0, r[2, 1] - r[1, 2])
    y = math.copysign(math.sqrt(max(0.0, 1.0 - r[0, 0] + r[1, 1] - r[2, 2])) / 2.0, r[0, 2] - r[2, 0])
    z = math.copysign(math.sqrt(max(0.0, 1.0 - r[0, 0] - r[1, 1] + r[2, 2])) / 2.0, r[1, 0] - r[0, 1])
    q = np.array([x, y, z, w])
    return (q / np.linalg.norm(q)).tolist()


def profile(kind, device, method, data, quality, depends_on=(), room=None):
    if room is not None:
        data = dict(data, room=room)
    return {
        "schema_version": 1,
        "id": f"camera-{kind}-{device}" + (f".{room}" if room else ""),
        "kind": f"camera_{kind}",
        "method": method,
        "origin": "live",
        "created_utc": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "subject": {"device_id": device, "segment_id": None},
        "depends_on": list(depends_on),
        "data": data,
        "quality": quality,
    }


def solve(directory, cols, rows, square_m, marker_ratio, origin_set=None, origin_height_m=0.0, log=print,
          room=None, lenses=None):
    """Returns (profiles, report). Raises ValueError with an actionable message.

    With `lenses` ({device: (K, dist, (w, h))}), intrinsics are reused instead of solved: only the
    room's extrinsics are returned, so another room needs one shared floor view, not a lens session.
    """
    board = make_board(cols, rows, square_m, square_m * marker_ratio)
    cameras = sorted(p for p in Path(directory).iterdir() if p.is_dir())
    if not cameras:
        raise ValueError(f"{directory} has no camera folders")
    detections, sizes = {}, {}
    for cam in cameras:
        detections[cam.name] = {}
        for f in sorted(cam.glob("*.png")) + sorted(cam.glob("*.jpg")):
            image = cv2.imread(str(f))
            sizes[cam.name] = (image.shape[1], image.shape[0])
            found = detect(board, image)
            if found:
                detections[cam.name][f.stem] = found
    profiles, report, intrinsics = [], {}, {}
    for name, views in detections.items():
        if lenses is not None:
            if name not in lenses:
                raise ValueError(f"{name}: the engine has no valid intrinsics; calibrate the lens first (without --room)")
            k, dist, size = lenses[name]
            if tuple(size) != tuple(sizes[name]):
                raise ValueError(f"{name}: images are {sizes[name]}, intrinsics are for {tuple(size)}")
            intrinsics[name] = (k, dist)
            report[name] = {}
            continue
        if len(views) < MIN_VIEWS:
            raise ValueError(f"{name}: board found in {len(views)} images; capture at least {MIN_VIEWS} "
                             f"with the board tilted across the whole view")
        obj = [v[0] for v in views.values()]
        img = [v[1] for v in views.values()]
        rms, k, dist, _, _ = cv2.calibrateCamera(obj, img, sizes[name], None, None)
        dist = dist.ravel().tolist()[:5] + [0.0] * max(0, 5 - dist.size)
        if rms > MAX_INTRINSIC_RMS_PX:
            raise ValueError(f"{name}: intrinsic RMS {rms:.2f} px exceeds {MAX_INTRINSIC_RMS_PX}; "
                             f"remove blurred images or capture more varied views")
        intrinsics[name] = (k, np.array(dist))
        log(f"{name}: intrinsics from {len(views)} views, RMS {rms:.3f} px, fx {k[0, 0]:.1f} fy {k[1, 1]:.1f}")
        profiles.append(profile("intrinsics", name, "charuco_opencv", {
            "image_size_px": list(sizes[name]), "fx_px": k[0, 0], "fy_px": k[1, 1],
            "cx_px": k[0, 2], "cy_px": k[1, 2], "distortion": dist,
        }, {"rms_px": rms, "views": len(views)}))
        report[name] = {"intrinsic_rms_px": rms}
    shared = set.intersection(*(set(v) for v in detections.values()))
    if origin_set is None:
        if not shared:
            raise ValueError("No image set shows the board to every camera; capture one with the board "
                             "flat on the floor where all cameras see it")
        origin_set = max(shared, key=lambda s: sum(len(detections[c][s][0]) for c in detections))
    elif origin_set not in shared:
        raise ValueError(f"set {origin_set} does not show the board to every camera")
    up = None
    for name in detections:
        obj, img = detections[name][origin_set]
        k, dist = intrinsics[name]
        ok, rvec, tvec = cv2.solvePnP(obj, img, k, dist, flags=cv2.SOLVEPNP_ITERATIVE)
        if not ok:
            raise ValueError(f"{name}: board pose could not be solved in set {origin_set}")
        projected, _ = cv2.projectPoints(obj, rvec, tvec, k, dist)
        pnp_rms = float(np.sqrt(np.mean(np.sum((projected.reshape(-1, 2) - img) ** 2, axis=1))))
        if pnp_rms > MAX_EXTRINSIC_RMS_PX:
            raise ValueError(f"{name}: board pose RMS {pnp_rms:.2f} px exceeds {MAX_EXTRINSIC_RMS_PX}")
        r_cb, _ = cv2.Rodrigues(rvec)
        centre_b = (-r_cb.T @ tvec).ravel()
        # The floor normal points towards the cameras: the board's +z or -z, decided once.
        if up is None:
            up = 1.0 if centre_b[2] > 0 else -1.0
        elif up * centre_b[2] <= 0:
            raise ValueError(f"{name} is on the other side of the board plane from the first camera")
        # Rows of m map board axes to world axes: X_w = x_b, Y_w = up * z_b, Z_w = X_w x Y_w.
        x_w, y_w = np.array([1.0, 0, 0]), np.array([0, 0, up])
        m = np.vstack([x_w, y_w, np.cross(x_w, y_w)])
        r_wc = m @ r_cb.T
        t_wc = m @ centre_b + np.array([0.0, origin_height_m, 0.0])
        log(f"{name}: extrinsics from set {origin_set}, RMS {pnp_rms:.3f} px, "
            f"position [{t_wc[0]:.3f}, {t_wc[1]:.3f}, {t_wc[2]:.3f}] m")
        profiles.append(profile("extrinsics", name, "charuco_opencv_floor_board", {
            "q_world_from_camera": rot_to_quat(r_wc), "t_world_from_camera_m": t_wc.tolist(),
        }, {"rms_px": pnp_rms, "corners": len(obj), "origin_set": origin_set},
            depends_on=[{"kind": "camera_intrinsics", "device_id": name}], room=room))
        report[name].update({"extrinsic_rms_px": pnp_rms, "position_m": t_wc.tolist(), "r_wc": r_wc})
    return profiles, report


def engine_lenses(engine):
    """Valid camera intrinsics the engine holds: {device: (K, dist, (w, h))}."""
    with urllib.request.urlopen(engine.rstrip("/") + "/api/v1/state", timeout=10) as response:
        state = json.load(response)["data"]
    out = {}
    for p in state["calibration"]:
        d = p.get("data") or {}
        if p["kind"] == "camera_intrinsics" and p["state"] == "valid" and d:
            k = np.array([[d["fx_px"], 0, d["cx_px"]], [0, d["fy_px"], d["cy_px"]], [0, 0, 1]])
            out[p["device_id"]] = (k, np.array(d["distortion"]), d["image_size_px"])
    return out


def cmd_solve(a):
    if a.room and not a.engine:
        sys.exit("--room reuses the engine's intrinsics: give --engine too")
    try:
        lenses = engine_lenses(a.engine) if a.room else None
        profiles, _ = solve(a.dir, a.cols, a.rows, a.square_m, a.marker_ratio, a.origin_set, a.origin_height_m,
                            room=a.room, lenses=lenses)
    except ValueError as e:
        sys.exit(f"calibration failed: {e}")
    if a.out:
        Path(a.out).write_text(json.dumps(profiles, indent=2))
        print(f"Wrote {len(profiles)} profiles to {a.out}")
    if a.engine:
        request = urllib.request.Request(a.engine.rstrip("/") + "/api/v1/calibration/profiles",
                                         data=json.dumps(profiles).encode(), method="POST",
                                         headers={"Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(request, timeout=10) as response:
                print(f"Engine accepted {len(json.load(response)['data'])} profiles")
        except urllib.error.HTTPError as e:
            sys.exit(f"engine rejected the profiles: {e.read().decode(errors='replace')}")


def look_at(position, target):
    """Camera rotation (rows: camera axes in world) with OpenCV axes and world +Y up."""
    forward = target - position
    forward /= np.linalg.norm(forward)
    right = np.cross(forward, [0.0, 1.0, 0.0])
    right /= np.linalg.norm(right)
    down = np.cross(forward, right)
    return np.vstack([right, down, forward])  # r_cw


def cmd_selftest(a):
    """Renders ChArUco views through known cameras and checks the solve recovers them."""
    import tempfile
    rng = np.random.default_rng(7)
    # A floor board must stay legible from across the room: 20 cm squares, a 1.4 x 1.0 m mat.
    cols, rows, square = 7, 5, 0.20
    board = make_board(cols, rows, square, square * 0.75)
    ppm = 1000  # board texture pixels per metre
    texture = board.generateImage((int(cols * square * ppm), int(rows * square * ppm)), marginSize=0)
    w, h = 1280, 720
    k = np.array([[900.0, 0, 640], [0, 900.0, 360], [0, 0, 1]])
    # World truth: board on the floor, its x along world X and its y along world +Z, so board
    # +z (= x cross y) points down and cameras above see its printed (-z) side, as OpenCV expects.
    b_to_w = np.array([[1.0, 0, 0], [0, 0, -1], [0, 1, 0]])
    centre = np.array([cols * square / 2, 0.0, rows * square / 2])
    truth = {"CAM1": np.array([0.3, 1.5, 2.6]), "CAM2": np.array([2.6, 1.8, 0.4]), "CAM3": np.array([-1.8, 1.2, -1.5])}

    def render(r_cb, t_cb):
        h_metric = k @ np.column_stack([r_cb[:, 0], r_cb[:, 1], t_cb])
        scale = np.diag([1.0 / ppm, 1.0 / ppm, 1.0])
        image = cv2.warpPerspective(texture, h_metric @ scale, (w, h), borderValue=255)
        return cv2.GaussianBlur(image, (3, 3), 0)

    with tempfile.TemporaryDirectory() as d:
        for name, position in truth.items():
            Path(d, name).mkdir()
            r_cw = look_at(position, centre)
            # Shared set 0000: the floor board as seen from the true pose.
            r_cb = r_cw @ b_to_w
            t_cb = r_cw @ (b_to_w @ np.zeros(3) - position)
            cv2.imwrite(str(Path(d, name, "0000.png")), render(r_cb, t_cb))
            # Intrinsic views: the board tilted in front of the camera, printed side towards it.
            for i in range(1, 16):
                rvec = rng.uniform(-0.5, 0.5, 3)
                r, _ = cv2.Rodrigues(rvec)
                t = np.array([rng.uniform(-0.9, 0.1), rng.uniform(-0.6, 0.0), rng.uniform(2.4, 4.0)])
                cv2.imwrite(str(Path(d, name, f"{i:04d}.png")), render(r, t))
        profiles, report = solve(d, cols, rows, square, 0.75, origin_set="0000", log=lambda m: None)
        # A second room reuses the lens models: only extrinsics, ids and data naming the room.
        lenses = {n: (k, np.zeros(5), (w, h)) for n in truth}
        room_profiles, room_report = solve(d, cols, rows, square, 0.75, origin_set="0000",
                                           log=lambda m: None, room="garage", lenses=lenses)
    failures = []
    if sorted(p["id"] for p in room_profiles) != [f"camera-extrinsics-{n}.garage" for n in sorted(truth)] \
            or any(p["data"]["room"] != "garage" for p in room_profiles):
        failures.append("room profiles")
    for name, position in truth.items():
        if np.linalg.norm(np.array(room_report[name]["position_m"]) - position) > 0.01 * np.linalg.norm(position - centre):
            failures.append(f"{name} (room)")
    for name, position in truth.items():
        got = np.array(report[name]["position_m"])
        err = np.linalg.norm(got - position)
        fx = next(p for p in profiles if p["id"] == f"camera-intrinsics-{name}")["data"]["fx_px"]
        r_wc = report[name]["r_wc"]
        angle = math.degrees(math.acos(np.clip((np.trace(r_wc @ look_at(position, centre)) - 1) / 2, -1, 1)))
        print(f"{name}: position error {err * 1000:.1f} mm, rotation error {angle:.3f} deg, fx {fx:.1f} (900)")
        # Depth error follows the focal-length error, so position is bounded relative to range.
        if err > 0.01 * np.linalg.norm(position - centre) or angle > 0.5 or abs(fx - 900) > 9:
            failures.append(name)
    if failures:
        sys.exit(f"selftest FAILED for {failures}")
    print("selftest passed")


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)

    def board_args(s):
        s.add_argument("--cols", type=int, default=7, help="squares across")
        s.add_argument("--rows", type=int, default=5, help="squares down")
        s.add_argument("--marker-ratio", type=float, default=0.75, help="marker edge / square edge")

    s = sub.add_parser("board")
    board_args(s)
    s.add_argument("--square-px", type=int, default=300)
    s.add_argument("--out", default="charuco-board.png")
    s.set_defaults(func=cmd_board)
    s = sub.add_parser("capture")
    board_args(s)
    s.add_argument("--camera", action="append", required=True, help="DEVICE_ID=index, repeatable")
    s.add_argument("--out", default="calib")
    s.add_argument("--width", type=int, default=1280)
    s.add_argument("--height", type=int, default=720)
    s.set_defaults(func=cmd_capture)
    s = sub.add_parser("solve")
    board_args(s)
    s.add_argument("--dir", default="calib", help="one folder per camera, named by device id")
    s.add_argument("--square-m", type=float, required=True, help="measured square edge, metres")
    s.add_argument("--origin-set", help="image name (without extension) of the shared floor view")
    s.add_argument("--origin-height-m", type=float, default=0.0, help="board height above the floor")
    s.add_argument("--engine", help="engine URL to import into, e.g. http://127.0.0.1:7700")
    s.add_argument("--out", help="also write the profiles to this JSON file")
    s.add_argument("--room", help="calibrate only this room's camera poses, reusing the engine's intrinsics "
                                  "(lowercase letters, digits, _ and -)")
    s.set_defaults(func=cmd_solve)
    s = sub.add_parser("selftest")
    s.set_defaults(func=cmd_selftest)
    a = p.parse_args()
    a.func(a)


if __name__ == "__main__":
    main()
