#!/bin/bash
# Stage 18.332 — Test runner script.
#
# Runs cargo test with an appropriate thread count based on system resources.
# Per §3.2 (验收流): the test suite must pass with cargo test --release.
#
# **Why this script exists**:
# On systems with limited RAM (≤4GB) or limited CPUs (≤2), `cargo test`'s
# default of 8 parallel test threads can oversubscribe the system:
#   - Each test thread may spawn a `landin-stage0 --run` subprocess.
#   - Each subprocess loads libLLVM-22.so (~200MB RSS).
#   - Each subprocess spawns `cc` (another ~50MB).
#   - Each subprocess then runs the compiled binary.
#   - 8 parallel × 250MB = 2GB just for the test harness, plus the cargo
#     test runner itself + system overhead = OOM-killer territory on 4GB RAM.
#
# Per §1.0 原則 4 (报错 > 静默): when the system runs out of memory, the OOM
# killer sends SIGKILL to landin-stage0 subprocesses, causing them to exit
# with signal (-1) and empty stdout/stderr. This is NOT a codegen bug —
# it's a system resource issue.
#
# Per §12 (最优 > 最小): the proper fix is to limit cargo test parallelism
# based on available system resources, not to disable the multi-threaded
# test path entirely.

set -e
cd "$(dirname "$0")/.."

# Source LLVM env.
source scripts/env.sh > /dev/null 2>&1 || true

# Determine thread count based on system resources.
# Cap at min(num_cpus, 4) to avoid oversubscription.
CPUS=$(nproc 2>/dev/null || echo 2)
MAX_THREADS=4
THREADS=$(( CPUS < MAX_THREADS ? CPUS : MAX_THREADS ))

# Also check available memory (in MB).
AVAIL_MB=$(free -m | awk '/^Mem:/ {print $7}')
# Each landin-stage0 + cc subprocess uses ~300MB. Don't exceed avail/300.
MEM_LIMIT=$(( AVAIL_MB / 300 ))
if [ "$MEM_LIMIT" -lt "$THREADS" ]; then
    THREADS=$MEM_LIMIT
fi
if [ "$THREADS" -lt 1 ]; then
    THREADS=1
fi

echo "info: running cargo test with --test-threads=$THREADS (cpus=$CPUS, avail=${AVAIL_MB}MB)"

exec cargo test --release --features llvm-backend -- --test-threads="$THREADS" "$@"
