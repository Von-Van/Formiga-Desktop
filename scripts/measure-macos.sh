#!/bin/bash
# measure-macos.sh - take one docs/PERFORMANCE.md measurement of a running Formiga process.
#
# Native CPU, energy and resident memory have to be sampled from the live overlay app. This is the
# sampling half of the procedure in docs/PERFORMANCE.md: set the scenario up by hand, run this, and
# paste the table row it prints. The states to measure are the rows of that document's table.
#
# No dependencies beyond what ships with macOS: ps, top, sysctl, sw_vers, system_profiler.
# Nothing here needs sudo, and nothing is written anywhere except an optional --out file.
#
#   scripts/measure-macos.sh --state "four moving, busy desktop" --build "v0.57.0" --colony 4

set -u

STATE=""
BUILD="unreleased"
COLONY="?"
NOTES=""
PROCESS="formiga"
PID=""
WARMUP=300
DURATION=600
INTERVAL=5
ENERGY=1
OUT=""

usage() {
  cat >&2 <<'USAGE'
usage: measure-macos.sh --state "<scenario>" [options]

  --state TEXT      Scenario label, e.g. "four moving, busy desktop, menu open". Required.
  --build TEXT      Build label for the results row, e.g. "v0.57.0 preview".
  --colony N        Number of creatures in the colony (1 or 4 for R08).
  --notes TEXT      Free-text notes column.
  --pid N           Sample this pid instead of looking one up.
  --process NAME    Process name to look up (default: formiga).
  --warmup SECS     Idle wait before sampling starts (default 300, the PERFORMANCE.md warm-up).
  --duration SECS   Length of the sample window (default 600, the PERFORMANCE.md sample).
  --interval SECS   Seconds between samples (default 5).
  --no-energy       Skip the per-sample energy-impact reading.
  --out FILE        Append the finished record to FILE as well as printing it.
  -h, --help        This message.

CPU is reported as percent of ONE core, the way Activity Monitor reports it, computed from the
process's cumulative CPU time so it is a true average over the window rather than a decayed
instantaneous guess.

Memory is reported twice. The budget in docs/PERFORMANCE.md is resident set size:
  * RSS            - `ps -o rss`. The budgeted number.
  * Phys footprint - what Activity Monitor's Memory column shows. It also counts the overlay's
                     IOSurface and GPU-owned pages, so it is recorded in the notes for context.
USAGE
}

while [ $# -gt 0 ]; do
  case "$1" in
    --state)    STATE="${2-}"; shift 2 ;;
    --build)    BUILD="${2-}"; shift 2 ;;
    --colony)   COLONY="${2-}"; shift 2 ;;
    --notes)    NOTES="${2-}"; shift 2 ;;
    --pid)      PID="${2-}"; shift 2 ;;
    --process)  PROCESS="${2-}"; shift 2 ;;
    --warmup)   WARMUP="${2-}"; shift 2 ;;
    --duration) DURATION="${2-}"; shift 2 ;;
    --interval) INTERVAL="${2-}"; shift 2 ;;
    --no-energy) ENERGY=0; shift ;;
    --out)      OUT="${2-}"; shift 2 ;;
    -h|--help)  usage; exit 0 ;;
    *) echo "measure-macos.sh: unknown argument '$1'" >&2; usage; exit 2 ;;
  esac
done

if [ -z "$STATE" ]; then
  echo "measure-macos.sh: --state is required (it becomes the State column)." >&2
  usage
  exit 2
fi

# ---------------------------------------------------------------- resolve the process

if [ -z "$PID" ]; then
  PID="$(pgrep -x "$PROCESS" | head -1)"
fi
if [ -z "$PID" ]; then
  PID="$(pgrep -f "$PROCESS" | head -1)"
fi
if [ -z "$PID" ]; then
  echo "measure-macos.sh: no running process named '$PROCESS'. Launch the app first, or pass --pid." >&2
  exit 1
fi
if ! ps -p "$PID" >/dev/null 2>&1; then
  echo "measure-macos.sh: pid $PID is not running." >&2
  exit 1
fi

PROC_NAME="$(ps -o comm= -p "$PID" | sed 's#.*/##')"

# ---------------------------------------------------------------- describe the machine

OS_NAME="$(sw_vers -productName 2>/dev/null)"
OS_VER="$(sw_vers -productVersion 2>/dev/null)"
OS_BUILD="$(sw_vers -buildVersion 2>/dev/null)"
CPU="$(sysctl -n machdep.cpu.brand_string 2>/dev/null)"
CORES="$(sysctl -n hw.ncpu 2>/dev/null)"
MEM_GB="$(( $(sysctl -n hw.memsize 2>/dev/null || echo 0) / 1073741824 ))"

DISPLAY_INFO="$(system_profiler SPDisplaysDataType 2>/dev/null)"
GPU="$(printf '%s\n' "$DISPLAY_INFO" | awk -F': ' '/Chipset Model/ {print $2; exit}')"
MONITORS="$(printf '%s\n' "$DISPLAY_INFO" | grep -c 'Resolution:')"
SCALING="$(printf '%s\n' "$DISPLAY_INFO" \
  | awk -F': ' '/Resolution:|UI Looks like:/ {gsub(/^ +/,"",$2); printf "%s; ", $2}')"
[ -z "$GPU" ] && GPU="unknown"
[ "$MONITORS" = "0" ] && MONITORS="unknown"
[ -z "$SCALING" ] && SCALING="unknown"

MACHINE="$CPU, $CORES cores, ${MEM_GB} GB, $OS_NAME $OS_VER ($OS_BUILD)"

# ---------------------------------------------------------------- sampling helpers

# Cumulative CPU seconds for the process. `ps -o time=` prints [[HH:]MM:]SS[.ss].
cpu_seconds() {
  ps -o time= -p "$PID" 2>/dev/null | awk '
    { t=$1; n=split(t,a,":"); s=0; for (i=1;i<=n;i++) s=s*60+a[i]; printf "%.2f", s }'
}

rss_kb() { ps -o rss= -p "$PID" 2>/dev/null | tr -d ' '; }

# One `top` call gives both the phys_footprint (its MEM column, the Activity Monitor number) and
# the energy impact. `top -l 1` always reports energy 0 because energy impact is a rate and a
# single sample has no interval, so this takes two samples a second apart and keeps the second.
# That costs about a second per sample; the CPU maths measures real elapsed wall time, so it stays
# correct regardless.
top_mem_and_power() {
  top -l 2 -s 1 -pid "$PID" -stats mem,power 2>/dev/null | awk '
    function tomb(v,   u,n) {
      u = substr(v, length(v), 1)
      n = substr(v, 1, length(v)-1) + 0
      if (u == "G") return n * 1024
      if (u == "M") return n
      if (u == "K") return n / 1024
      if (u == "B") return n / 1048576
      return v + 0
    }
    /^[0-9]/ && NF == 2 { mem = tomb($1); pwr = $2 + 0 }
    END { printf "%.1f %.2f", mem, pwr }'
}

now_seconds() { date +%s; }

# ---------------------------------------------------------------- warm-up

echo "measure-macos.sh: pid $PID ($PROC_NAME), state '$STATE'" >&2
if [ "$WARMUP" -gt 0 ]; then
  echo "measure-macos.sh: warm-up ${WARMUP}s - leave the scenario exactly as it is." >&2
  remaining="$WARMUP"
  while [ "$remaining" -gt 0 ]; do
    step=30
    [ "$remaining" -lt 30 ] && step="$remaining"
    sleep "$step"
    remaining=$(( remaining - step ))
    if ! ps -p "$PID" >/dev/null 2>&1; then
      echo "measure-macos.sh: process $PID exited during warm-up; nothing was measured." >&2
      exit 1
    fi
    [ "$remaining" -gt 0 ] && echo "measure-macos.sh: warm-up ${remaining}s remaining" >&2
  done
fi

# ---------------------------------------------------------------- sample window

echo "measure-macos.sh: sampling ${DURATION}s at ${INTERVAL}s intervals" >&2

RAW="$(mktemp -t formiga-sample)"
trap 'rm -f "$RAW"' EXIT

start_wall="$(now_seconds)"
start_cpu="$(cpu_seconds)"
prev_wall="$start_wall"
prev_cpu="$start_cpu"
samples=0

while :; do
  sleep "$INTERVAL"
  if ! ps -p "$PID" >/dev/null 2>&1; then
    echo "measure-macos.sh: process $PID exited after ${samples} samples; reporting what was collected." >&2
    break
  fi
  wall="$(now_seconds)"
  cpu="$(cpu_seconds)"
  rss="$(rss_kb)"
  if [ "$ENERGY" = "1" ]; then
    read -r foot pwr <<EOF
$(top_mem_and_power)
EOF
  else
    foot=0
    pwr=0
  fi
  [ -z "$rss" ] && rss=0
  [ -z "${foot:-}" ] && foot=0
  [ -z "${pwr:-}" ] && pwr=0
  dw=$(( wall - prev_wall ))
  [ "$dw" -le 0 ] && dw=1
  pct="$(awk -v c="$cpu" -v p="$prev_cpu" -v w="$dw" 'BEGIN { d=c-p; if (d<0) d=0; printf "%.3f", d*100.0/w }')"
  printf '%s %s %s %s\n' "$pct" "$rss" "$pwr" "$foot" >> "$RAW"
  samples=$(( samples + 1 ))
  prev_wall="$wall"
  prev_cpu="$cpu"
  elapsed=$(( wall - start_wall ))
  if [ $(( samples % 12 )) -eq 0 ]; then
    echo "measure-macos.sh: ${elapsed}s / ${DURATION}s, last ${pct}% CPU, $(( rss / 1024 )) MB RSS, ${foot} MB footprint" >&2
  fi
  [ "$elapsed" -ge "$DURATION" ] && break
done

end_wall="$(now_seconds)"
end_cpu="$(cpu_seconds)"
window=$(( end_wall - start_wall ))
[ "$window" -le 0 ] && window=1

if [ "$samples" -eq 0 ]; then
  echo "measure-macos.sh: no samples collected." >&2
  exit 1
fi

read -r CPU_AVG CPU_PEAK RSS_AVG RSS_PEAK PWR_AVG PWR_PEAK FOOT_AVG FOOT_PEAK <<EOF
$(awk -v total_cpu="$(awk -v a="$end_cpu" -v b="$start_cpu" 'BEGIN{printf "%.2f", a-b}')" \
      -v window="$window" '
  {
    n++
    if ($1 > cpu_peak) cpu_peak = $1
    rss_sum  += $2; if ($2 > rss_peak)  rss_peak  = $2
    pwr_sum  += $3; if ($3 > pwr_peak)  pwr_peak  = $3
    foot_sum += $4; if ($4 > foot_peak) foot_peak = $4
  }
  END {
    printf "%.2f %.2f %.1f %.1f %.2f %.2f %.1f %.1f",
      total_cpu * 100.0 / window, cpu_peak,
      rss_sum / n / 1024.0, rss_peak / 1024.0,
      pwr_sum / n, pwr_peak,
      foot_sum / n, foot_peak
  }' "$RAW")
EOF

STATUS="recorded"

RECORD="$(cat <<EOF
--------------------------------------------------------------------------
Formiga native measurement (PERFORMANCE.md protocol)
--------------------------------------------------------------------------
  Build            : $BUILD
  Binary / pid     : $PROC_NAME / $PID
  OS               : $OS_NAME $OS_VER ($OS_BUILD)
  CPU              : $CPU ($CORES cores)
  GPU              : $GPU
  Memory installed : ${MEM_GB} GB
  Monitors         : $MONITORS
  Monitor scaling  : $SCALING
  Colony size      : $COLONY
  Activity state   : $STATE
  Warm-up          : ${WARMUP}s
  Sample window    : ${window}s wall, ${samples} samples at ${INTERVAL}s
  CPU average      : ${CPU_AVG}% of one core
  CPU peak         : ${CPU_PEAK}% of one core (worst single interval)
  Resident (RSS)   : ${RSS_AVG} MB average, ${RSS_PEAK} MB peak     <- ps -o rss
  Phys footprint   : ${FOOT_AVG} MB average, ${FOOT_PEAK} MB peak     <- Activity Monitor "Memory"
  Energy impact    : ${PWR_AVG} average, ${PWR_PEAK} peak (macOS per-process energy impact)
  Notes            : $NOTES

Budget check (docs/PERFORMANCE.md): resting colony < 1% CPU, four moving < 3% CPU,
resident memory (RSS) < 100 MB, presentation <= 20 fps, no busy loop while paused.
Presentation rate and a quiet paused loop are visual checks this script cannot make.

Paste into the PERFORMANCE.md table:

| $BUILD | $MACHINE | $STATE | $COLONY | ${CPU_AVG}% | ${CPU_PEAK}% | ${RSS_PEAK} MB | ${PWR_AVG} | $STATUS | footprint ${FOOT_PEAK} MB; ${WARMUP}s warm-up, ${window}s sample. $NOTES |
--------------------------------------------------------------------------
EOF
)"

printf '%s\n' "$RECORD"

if [ -n "$OUT" ]; then
  printf '%s\n\n' "$RECORD" >> "$OUT"
  echo "measure-macos.sh: appended to $OUT" >&2
fi
