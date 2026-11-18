#!/usr/bin/env python3
"""Turns repetition and phase marks read off a phone video into RepAnnotation JSON (martial-arts
20 §4.1) on the engine's session clock, for V-19 and V-20 (16 §5). Standard library only; run
offline by the annotator, never by the engine.

Input CSV, one row per phase of each repetition, frame numbers of the phone video:

    rep,phase,start_frame,end_frame
    1,chamber,1032,1101
    1,extension,1101,1146
    1,retraction,1146,1190
    1,recovery,1190,1302
    2,chamber,1871,1939
    ...

Optional extra row per repetition: `rep,strike,<frame>,<frame>` for the strike instant, and
`rep,quality,<positive|variation|error|unusable>,` to override the quality (default positive).
Phase names are the technique's (taekwondo.dollyeo_chagi: chamber, pivot, extension, retraction,
recovery); a repetition's interval is its first phase start to its last phase end.

Time alignment: a clap seen by both the phone and the webcam. `--sync-frame` is the phone frame
in which the hands meet; `--sync-us` the session time of the same clap, printed by
`bansayo-engine claps <data_dir>/sessions/<id>`. An LED driven at a known session time works the
same way.

    python scripts/annotate_reps.py marks.csv --session 20261203T180000Z-k9x2 --fps 240 \\
        --sync-frame 412 --sync-us 4180233 --annotator "A. Coach" \\
        --label taekwondo.dollyeo_chagi --side right --out annotations.json

Then copy annotations.json into the promoted golden session directory (owner checklist).
`python scripts/annotate_reps.py --selftest` checks the conversion.
"""
import argparse
import csv
import io
import json
import sys


def to_us(frame, fps, sync_frame, sync_us):
    return int(round(sync_us + (frame - sync_frame) / fps * 1e6))


def convert(rows, fps, sync_frame, sync_us, annotator, label, side):
    reps = {}
    for r in rows:
        rep = int(r["rep"])
        a = reps.setdefault(rep, {"phases": [], "strike": None, "quality": "positive"})
        phase = r["phase"].strip()
        if phase == "strike":
            a["strike"] = to_us(float(r["start_frame"]), fps, sync_frame, sync_us)
        elif phase == "quality":
            a["quality"] = r["start_frame"].strip()
        else:
            start = to_us(float(r["start_frame"]), fps, sync_frame, sync_us)
            end = to_us(float(r["end_frame"]), fps, sync_frame, sync_us)
            if end <= start:
                raise ValueError(f"rep {rep} {phase}: end frame must follow start frame")
            a["phases"].append({"phase": phase, "t_start_us": start, "t_end_us": end})
    out = []
    for rep in sorted(reps):
        a = reps[rep]
        if not a["phases"]:
            raise ValueError(f"rep {rep}: no phases")
        out.append({
            "schema_version": 1,
            "annotator": annotator,
            "label": label,
            "side": side,
            "phases": sorted(a["phases"], key=lambda p: p["t_start_us"]),
            "strike_t_us": a["strike"],
            "quality": a["quality"],
            "variation": None,
            "error_tags": [],
            "cue_agreement": [],
            "note": f"rep {rep} from video at {fps} fps, clap-aligned",
        })
    return out


def selftest():
    text = "rep,phase,start_frame,end_frame\n1,chamber,240,288\n1,extension,288,312\n1,strike,300,300\n"
    out = convert(list(csv.DictReader(io.StringIO(text))), 240.0, 240, 5_000_000, "t", "x", "right")
    assert out[0]["phases"][0] == {"phase": "chamber", "t_start_us": 5_000_000, "t_end_us": 5_200_000}
    assert out[0]["phases"][1]["t_end_us"] == 5_300_000
    assert out[0]["strike_t_us"] == 5_250_000
    print("selftest ok")


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("csv", nargs="?")
    p.add_argument("--session")
    p.add_argument("--fps", type=float, default=240.0)
    p.add_argument("--sync-frame", type=float)
    p.add_argument("--sync-us", type=int)
    p.add_argument("--annotator")
    p.add_argument("--label", default="taekwondo.dollyeo_chagi")
    p.add_argument("--side", choices=["left", "right"])
    p.add_argument("--out", default="annotations.json")
    p.add_argument("--selftest", action="store_true")
    a = p.parse_args()
    if a.selftest:
        return selftest()
    missing = [n for n in ("csv", "session", "sync_frame", "sync_us", "annotator") if getattr(a, n) is None]
    if missing:
        p.error("missing: " + ", ".join(missing))
    with open(a.csv, newline="", encoding="utf-8") as f:
        annotations = convert(list(csv.DictReader(f)), a.fps, a.sync_frame, a.sync_us, a.annotator, a.label, a.side)
    with open(a.out, "w", encoding="utf-8") as f:
        json.dump({"schema": "bansayo.annotations/v1", "session": a.session, "annotations": annotations}, f, indent=2)
    print(f"{len(annotations)} repetitions written to {a.out}")


if __name__ == "__main__":
    sys.exit(main())
