#!/bin/bash
# Stage 111 — stable test runner with per-run logging.
set -uo pipefail
N="${1:-3}"
cd "$(dirname "$0")/.."
source scripts/env.sh 2>/dev/null
ulimit -s unlimited
export PATH="$HOME/.cargo/bin:$PATH"

SUMMARY=/tmp/stage111_summary.txt
echo "=== Stage 111 Stability Test (N=$N) ===" > "$SUMMARY"
echo "Started: $(date)" >> "$SUMMARY"
PASS=0
FAIL=0

for i in $(seq 1 "$N"); do
    # Each run outputs to its own log; we extract only the result line.
    RUN_LOG=/tmp/stage111_run_$i.log
    cargo test --release --features llvm-backend --test all_tests > "$RUN_LOG" 2>&1
    RESULT=$(grep "test result:" "$RUN_LOG" | tail -1)
    if echo "$RESULT" | grep -q "0 failed"; then
        echo "Run $i: PASS — $RESULT" >> "$SUMMARY"
        PASS=$((PASS + 1))
    else
        echo "Run $i: FAIL — $RESULT" >> "$SUMMARY"
        grep "^test .*FAILED" "$RUN_LOG" | head -5 >> "$SUMMARY"
        FAIL=$((FAIL + 1))
    fi
    rm -f "$RUN_LOG"
done

echo "" >> "$SUMMARY"
echo "=== Summary ===" >> "$SUMMARY"
echo "Pass: $PASS / $N, Fail: $FAIL / $N" >> "$SUMMARY"
echo "Finished: $(date)" >> "$SUMMARY"
cat "$SUMMARY"
exit $FAIL
