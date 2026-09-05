#!/bin/bash
# Stage 111 — minimal stability summary (compact output).
set -uo pipefail
N="${1:-10}"
cd "$(dirname "$0")/.."
source scripts/env.sh
ulimit -s unlimited
export PATH="$HOME/.cargo/bin:$PATH"

PASS=0
FAIL=0

for i in $(seq 1 "$N"); do
    OUTPUT=$(cargo test --release --features llvm-backend --test all_tests 2>&1)
    RESULT=$(echo "$OUTPUT" | grep "test result:" | tail -1)
    if echo "$RESULT" | grep -q "0 failed"; then
        PASS=$((PASS + 1))
    else
        FAIL=$((FAIL + 1))
    fi
done

echo "Pass: $PASS / $N, Fail: $FAIL / $N"
exit $FAIL
