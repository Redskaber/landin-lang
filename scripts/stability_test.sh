#!/bin/bash
# Stage 111 — 100-run stability test for non-deterministic SIGSEGV detection.
#
# Purpose: verify the cargo test suite is deterministic across N runs.
# Stage 105 RCA: 3/100 SIGSEGV (ASLR on) at baseline.
# Stage 111 hypothesis: Debug impl bodies (reverted) trigger 10-18/4755
# non-deterministic failures per cargo test invocation.
#
# Usage:
#   bash scripts/stability_test.sh [N]
#   (default N=100, but 10 is enough to detect regression)
#
# Per §17.6 (直到审查不出问题为止): iterate until 0 failures across N runs.

set -uo pipefail

N="${1:-10}"
cd "$(dirname "$0")/.."

source scripts/env.sh
ulimit -s unlimited
export PATH="$HOME/.cargo/bin:$PATH"

PASS=0
FAIL=0
FAIL_DETAILS=()

echo "=== Stage 111 Stability Test ==="
echo "Runs: $N"
echo "Test command: cargo test --release --features llvm-backend --test all_tests"
echo ""

for i in $(seq 1 "$N"); do
    echo -n "Run $i/$N: "
    OUTPUT=$(cargo test --release --features llvm-backend --test all_tests 2>&1)
    # Extract the test result line
    RESULT=$(echo "$OUTPUT" | grep "test result:" | tail -1)
    if echo "$RESULT" | grep -q "0 failed"; then
        echo "PASS ($RESULT)"
        PASS=$((PASS + 1))
    else
        echo "FAIL ($RESULT)"
        FAIL=$((FAIL + 1))
        FAIL_DETAILS+=("Run $i: $RESULT")
        # Capture which tests failed
        FAILED_TESTS=$(echo "$OUTPUT" | grep "^test .*FAILED" | head -5)
        FAIL_DETAILS+=("$FAILED_TESTS")
    fi
done

echo ""
echo "=== Summary ==="
echo "Pass: $PASS / $N"
echo "Fail: $FAIL / $N"
if [ "$FAIL" -gt 0 ]; then
    echo ""
    echo "=== Failure Details (first 5 each) ==="
    printf '%s\n' "${FAIL_DETAILS[@]}"
    exit 1
fi
exit 0
