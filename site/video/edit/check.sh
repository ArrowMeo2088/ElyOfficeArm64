#!/usr/bin/env bash
# Pulls stills at boundaries and mid-sections for review.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
film="$here/../out/ElyOffice-launch.mp4"
out="$here/../out/edit/check"
rm -rf "$out"
mkdir -p "$out"
ffprobe -v error -show_entries format=duration:stream=codec_name,profile,width,height,r_frame_rate,pix_fmt -of compact "$film"
# Frames: scene ends, plate mids, cuts, inner cuts, section ends.
frames=(0 120 257 258 400 514 515 545 578 579 640 700 771 772 805 835 836 870 899 900 960 1028 1029 1060 1092 1093 1120 1141 1142 1180 1221 1222 1260 1285
  1286 1320 1349 1350 1400 1450 1542 1543 1575 1607 1608 1630 1655 1656 1700 1799 1800 1835 1864 1865 1950 2057 2058 2200 2315 2450 2572 2699)
expr=$(printf "+eq(n\\,%s)" "${frames[@]}")
ffmpeg -v error -y -i "$film" -vf "select='${expr:1}'" -fps_mode passthrough -frame_pts 1 "$out/%05d.png"
ls "$out" | wc -l
