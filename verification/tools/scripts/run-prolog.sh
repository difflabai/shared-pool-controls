#!/usr/bin/env bash
# Run Prolog verification tests for a project.
#
# Usage: ./tools/scripts/run-prolog.sh <project-name> [test-file]
# Example: ./tools/scripts/run-prolog.sh ironclaw
# Example: ./tools/scripts/run-prolog.sh ironclaw test_sanitizer.pl

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

PROJECT="${1:?Usage: run-prolog.sh <project-name> [test-file]}"
TEST_FILE="${2:-}"
PROLOG_DIR="$ROOT_DIR/projects/$PROJECT/prolog"

if [ ! -d "$PROLOG_DIR" ]; then
    echo "Error: no prolog directory at $PROLOG_DIR"
    exit 1
fi

# Check for scryer-prolog binary
if ! command -v scryer-prolog &> /dev/null; then
    echo "Error: scryer-prolog not found in PATH"
    echo "Install with: cargo install scryer-prolog"
    exit 1
fi

# run_one <file>: run_tests must succeed. A failure, an exception or a
# goal that falls through to the toplevel (stdin is /dev/null) exits non-zero.
run_one() {
    scryer-prolog "$1" -g "(catch(run_tests, E, (write(E), nl, halt(2))) -> halt(0) ; halt(1))" </dev/null
}

echo "=== Running Prolog verification for $PROJECT ==="

if [ -n "$TEST_FILE" ]; then
    # Run specific test file
    TEST_PATH="$PROLOG_DIR/tests/$TEST_FILE"
    if [ ! -f "$TEST_PATH" ]; then
        echo "Error: test file not found at $TEST_PATH"
        exit 1
    fi
    echo "Test: $TEST_PATH"
    echo ""
    cd "$PROLOG_DIR/tests"
    run_one "$TEST_FILE"
    exit $?
fi

# Run all test files; exit non-zero if any fails.
status=0
for test in "$PROLOG_DIR/tests/"*.pl; do
    if [ -f "$test" ]; then
        echo "--- $(basename "$test") ---"
        cd "$PROLOG_DIR/tests"
        run_one "$(basename "$test")" || status=1
        echo ""
    fi
done

echo "=== Prolog verification complete ==="
exit $status
