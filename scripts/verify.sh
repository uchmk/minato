#!/usr/bin/env bash
# Every check a push to main must pass. The first failure stops with its
# output; the last line is `ALL OK: ...` when everything passed. Like
# tsumugi's scripts/verify.sh.
#
# The Windows clippy is a type check only -- it never runs Windows code, so
# read path strings by eye. Off Windows it is left to CI: ring (rustls, under
# ureq) builds its C for MSVC only with the MSVC tools. Never add `cargo fmt`
# here.

set -o pipefail
cd "$(dirname "$0")/.." || exit 1

case "$(uname -s)" in
    MINGW* | MSYS* | CYGWIN*) windows=1 ;;
    *) windows= ;;
esac

run() {
    local out rc
    out=$("$@" 2>&1)
    rc=$?
    if [ $rc -ne 0 ]; then
        echo "FAILED: $*"
        echo "$out" | tail -30
        exit 1
    fi
    last=$out
}

run cargo build -q --locked --workspace --all-features
run cargo test -q --workspace --all-features
tests=$(echo "$last" | grep 'test result' | awk '{p+=$4; f+=$6} END {print p " passed; " f " failed"}')
run cargo +stable clippy -q --workspace --all-targets --all-features -- -D warnings
if [ -n "$windows" ]; then
    run cargo +stable clippy -q --workspace --all-targets --all-features --target x86_64-pc-windows-msvc -- -D warnings
else
    echo "skipped: the Windows clippy (not on Windows; CI's clippy-windows runs it)"
fi
run cargo +stable clippy -q --workspace --all-targets -- -D warnings
# The checklist follows TESTING.md.
run cargo run -q -p minato --example make-testcheck -- --check
echo "ALL OK: $tests"
