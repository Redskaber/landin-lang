#!/bin/bash
# Stage 111 — minimal stability summary, output to log file (avoid MCP SSE limit).
set -uo pipefail
N="${1:-5}"
cd "$(dirname "$0")/.."
source scripts/env.sh
ulimit -s unlimited
export PATH="$HOME/.cargo/bin:$PATH"

LOG=/tmp/stage111_stability.log
PASS=0
FAIL=0
echo "=== Stage 111 Stability Test (N=$N) ===" > "$LOG"

for i in $(seq 1 "$N"); do
    echo "Run $i/$N..." >> "$LOG"
    OUTPUT=$(cargo test --release --features llvm-backend --test all_tests 2>&1)
    RESULT=$(echo "$OUTPUT" | grep "test result:" | tail -1)
    if echo "$RESULT" | grep -q "0 failed"; then
        echo "  PASS: $RESULT" >> "$LOG"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $RESULT" >> "$LOG"
        FAIL=$((FAIL + 1))
    fi
done

echo "" >> "$LOG"
echo "=== Summary ===" >> "$LOG"
echo "Pass: $PASS / $N, Fail: $FAIL / $N" >> "$LOG"
cat "$LOG" | tail -10
echo "(full log: $LOG)"
exit $FAIL
