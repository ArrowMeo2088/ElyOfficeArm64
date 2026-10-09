#!/usr/bin/env python3
"""Composites the launch film from scenes, recordings and captions."""
import json
import math
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE.parent / "out"
SCENES = OUT / "scenes"
REC = OUT / "rec"
WORK = OUT / "edit"
FILM = OUT / "ElyOffice-launch.mp4"
FPS = 30
BEAT = 60 / 112
RGB_IN = "scale=in_color_matrix=bt709:in_range=tv,format=rgb24"


def run(args):
    print("+", " ".join(str(a) for a in args[:6]), "...", flush=True)
    subprocess.run(["ffmpeg", "-hide_banner", "-v", "error", "-y", *map(str, args)], check=True)


def lerp(points, x):
    """Piecewise linear through points, extended past both ends."""
    i = 0
    while i < len(points) - 2 and x > points[i + 1][0]:
        i += 1
    (x0, y0), (x1, y1) = points[i], points[i + 1]
    return y0 + (x - x0) * (y1 - y0) / (x1 - x0)


def piecewise(points, var):
    """An ffmpeg expression for lerp over var."""
    expr = None
    for i in range(len(points) - 2, -1, -1):
        (x0, y0), (x1, y1) = points[i], points[i + 1]
        seg = f"({y0:.6f}+({var}-{x0:.6f})*{(y1 - y0) / (x1 - x0):.6f})"
        expr = seg if expr is None else f"if(lt({var},{x1:.6f}),{seg},{expr})"
    return expr


def frames_expr(values):
    """Per-frame value by input frame count."""
    return "+".join(f"eq(in,{k})*{v:.3f}" for k, v in enumerate(values))


def piece_chain(label, piece, start, f0, f1, out):
    """Retimes one recording piece into frames f0..f1 of a section."""
    anchors = [((start + b) * BEAT, s) for b, s in piece["anchors"]]
    inverse = [(s, t) for t, s in anchors]
    u0 = max(lerp(anchors, f0 / FPS) - 0.1, 0)
    u1 = lerp(anchors, f1 / FPS) + 0.1
    pts = piecewise(inverse, f"({u0:.6f}+T)")
    n = f1 - f0
    return (
        f"[{label}:v]trim=start={u0:.4f}:end={u1:.4f},setpts=PTS-STARTPTS,"
        f"setpts='({pts}-{f0 / FPS:.6f})/TB',fps={FPS}:start_time=0,"
        f"trim=end_frame={n},tpad=stop_mode=clone:stop={n},trim=end_frame={n},setpts=N/{FPS}/TB,"
        f"{RGB_IN},setsar=1[{out}]"
    )


def section(s, scene, end, plan):
    """Renders one app section to a lossless file."""
    app, first, n_plate = s["app"], scene["first"], scene["frames"]
    length = end - first
    start = scene["bars"][0] * 4 - first / FPS / BEAT
    inputs, chains, parts = [], [], []
    bounds = []
    for i, p in enumerate(s["pieces"]):
        a = p["anchors"]
        f0 = 0 if i == 0 else math.ceil((start + a[0][0]) * BEAT * FPS - 1e-6)
        bounds.append(f0)
    bounds.append(length)
    for i, p in enumerate(s["pieces"]):
        inputs += ["-i", REC / f"{p['src']}.mp4"]
        chains.append(piece_chain(i, p, start, bounds[i], bounds[i + 1], f"p{i}"))
        parts.append(f"[p{i}]")
    k = len(inputs) // 2
    inputs += ["-framerate", FPS, "-i", SCENES / scene["name"] / "%05d.png"]
    inputs += ["-loop", 1, "-framerate", FPS, "-i", WORK / "cards" / "frame.png"]
    inputs += ["-loop", 1, "-framerate", FPS, "-i", WORK / "cards" / f"{app}.png"]
    chains.append(f"{''.join(parts)}concat=n={len(parts)}:v=1:a=0[rec]")
    chains.append(f"[rec]split[ra][rb]")
    chains.append(plate_chain(scene, f"[ra]trim=end_frame={n_plate},setpts=PTS-STARTPTS", k, "plate"))
    chains.append(full_chain(s, plan, f"[rb]trim=start_frame={n_plate},setpts=PTS-STARTPTS", length - n_plate, k, "full"))
    chains.append("[plate][full]concat=n=2:v=1:a=0,format=rgb24[v]")
    target = WORK / f"{app}.mkv"
    run([*inputs, "-filter_complex", ";".join(chains), "-map", "[v]", "-frames:v", length,
         "-c:v", "libx264rgb", "-qp", 0, "-preset", "ultrafast", target])
    return target


def plate_chain(scene, rec, k, out):
    """Pins the recording onto the plate's screen face."""
    data = json.loads((SCENES / f"{scene['name']}.json").read_text())["frames"]
    landed = next(f["corners"] for f in data if f["corners"])
    corners = [f["corners"] or landed for f in data]
    pins = ":".join(
        f"{axis}{c}='{frames_expr([fr[c][j] for fr in corners])}'"
        for c in range(4) for j, axis in enumerate("xy")
    )
    fades = "".join(
        f",colorchannelmixer=aa={f['opacity']:.3f}:enable='eq(n,{f['local']})'"
        for f in data if f["opacity"] < 0.999
    )
    return (
        f"{rec},scale=800:450:flags=area,scale=1912:1072:flags=bicubic,format=rgba,"
        f"pad=1920:1080:4:4:color=black@0,setsar=1,perspective={pins}:sense=destination:eval=frame:interpolation=cubic"
        f"{fades}[pin];[{k}:v]format=rgba[bg];[bg][pin]overlay=format=rgb:shortest=1,format=rgb24,setsar=1[{out}]"
    )


def full_chain(s, plan, rec, n, k, out):
    """Letterboxes the recording with a slow push and the caption."""
    w = plan["window"]
    fx, fy = s["focus"]
    z = f"(1+{plan['push'] - 1:.4f}*in/{n - 1})"
    cw, ch = f"(1920/{z})", f"(1080/{z})"
    left = f"clip({fx}-{cw}/2,0,1920-{cw})"
    top = f"clip({fy}-{ch}/2,0,1080-{ch})"
    quad = (f"x0='{left}':y0='{top}':x1='{left}+{cw}':y1='{top}':"
            f"x2='{left}':y2='{top}+{ch}':x3='{left}+{cw}':y3='{top}+{ch}'")
    rise = "round(12*pow(max(0,1-t/0.2),2))"
    return (
        f"{rec},perspective={quad}:sense=source:eval=frame:interpolation=cubic,"
        f"scale={w['w']}:{w['h']}:flags=lanczos,pad=1920:1080:{w['x']}:{w['y']}:color=0xfbfbfa,setsar=1,format=rgba[lb];"
        f"[{k + 1}:v]format=rgba[fr];[lb][fr]overlay=format=rgb:shortest=0[lf];"
        f"[{k + 2}:v]format=rgba,fade=in:st=0:d=0.2:alpha=1[cap];"
        f"[lf][cap]overlay=x=0:y='{rise}':format=rgb:eval=frame,trim=end_frame={n},format=rgb24,setsar=1[{out}]"
    )


def scene_input(name):
    return ["-framerate", FPS, "-i", SCENES / name / "%05d.png"]


def film(made):
    """Joins every part and the music into the final MP4."""
    order = ["opening", "rise", *made, "files", "props", "end"]
    inputs = []
    for part in order:
        inputs += ["-i", part] if isinstance(part, Path) else scene_input(part)
    inputs += ["-i", OUT / "music.wav"]
    joined = "".join(f"[{i}:v]" for i in range(len(order)))
    graph = (f"{joined}concat=n={len(order)}:v=1:a=0,"
             "scale=out_color_matrix=bt709:out_range=tv,format=yuv420p[v]")
    run([*inputs, "-filter_complex", graph, "-map", "[v]", "-map", f"{len(order)}:a",
         "-frames:v", 2700, "-c:v", "libx264", "-profile:v", "high", "-preset", "slow", "-crf", 18,
         "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709", "-color_range", "tv",
         "-r", FPS, "-c:a", "aac", "-b:a", "192k", "-t", "90", "-movflags", "+faststart", FILM])


def main():
    plan = json.loads((HERE / "plan.json").read_text())
    scenes = json.loads((SCENES / "scenes.json").read_text())["scenes"]
    WORK.mkdir(parents=True, exist_ok=True)
    subprocess.run(["node", HERE / "cards.mjs"], check=True)
    only = sys.argv[1:]
    made = []
    for s in plan["sections"]:
        i = next(j for j, sc in enumerate(scenes) if sc["name"] == f"plate-{s['app']}")
        target = WORK / f"{s['app']}.mkv"
        if not only or s["app"] in only:
            target = section(s, scenes[i], scenes[i + 1]["first"], plan)
        made.append(target)
    if not only:
        film(made)


if __name__ == "__main__":
    main()
