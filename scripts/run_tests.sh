#!/bin/bash
# Stage 18.333 — Test runner script.
#
# Runs cargo test with an appropriate thread count based on system resources
# AND ensures adequate stack size for LLVM's recursive optimization passes.
#
# Per §3.2 (验收流): the test suite must pass with cargo test --release.
#
# **Why this script exists**:
# 1. On systems with limited RAM (≤4GB) or limited CPUs (≤2), cargo test's
#    default of 8 parallel test threads can oversubscribe the system (RAM
#    exhaustion → OOM killer → SIGKILL on landin-stage0 subprocesses).
# 2. LLVM's CodeGenPrepare / LowerFormalArguments / Prologepilog passes are
#    recursive on complex IR. The default 8MB stack size (ulimit -s 8192)
#    is too small — intermittent segfault at address 8 (NULL pointer deref
#    when the stack overflows into the guard page).
# 3. By setting `ulimit -s unlimited` (or 65536 = 64MB), the segfault
#    disappears — verified by 100/100 stable --emit-obj runs at
#    ulimit -s unlimited vs ~2% segfault rate at ulimit -s 8192.
#
# Per §1.0 原則 4 (报错 > 静默): the segfault is silent (no error message).
# Per §12 (最优 > 最小): the proper fix is to raise the stack limit, not
#   to disable LLVM's optimization passes (which would regress codegen
#   correctness — see Stage 18.329 LLVMCodeGenLevelDefault rationale).
# Per §1.0 原則 6 (通解 > 特解): one wrapper script for all test invocations.

set -e
cd "$(dirname "$0")/.."

# Source LLVM env.
source scripts/env.sh >/dev/null 2>&1 || true

# Stage 18.333: Raise stack limit to unlimited (or 64MB if unlimited fails).
# LLVM's recursive optimization passes need more than the default 8MB.
if ulimit -s unlimited 2>/dev/null; then
  : # unlimited worked
elif ulimit -s 65536 2>/dev/null; then
  : # 64MB worked
else
  echo "warning: cannot raise stack limit; LLVM may intermittently segfault" >&2
fi

# Determine thread count based on system resources.
# Cap at min(num_cpus, 4) to avoid oversubscription.
CPUS=$(nproc 2>/dev/null || echo 2)
MAX_THREADS=4
THREADS=$((CPUS < MAX_THREADS ? CPUS : MAX_THREADS))

# Also check available memory (in MB).
AVAIL_MB=$(free -m | awk '/^Mem:/ {print $7}')
# Each landin-stage0 + cc subprocess uses ~300MB. Don't exceed avail/300.
MEM_LIMIT=$((AVAIL_MB / 300))
if [ "$MEM_LIMIT" -lt "$THREADS" ]; then
  THREADS=$MEM_LIMIT
fi
if [ "$THREADS" -lt 1 ]; then
  THREADS=1
fi

echo "info: running cargo test with --test-threads=$THREADS (cpus=$CPUS, avail=${AVAIL_MB}MB, stack=$(ulimit -s))"

exec cargo test --release --features llvm-backend -- --test-threads="$THREADS" "$@"
