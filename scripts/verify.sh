#!/usr/bin/env bash
# Every check a push to main must pass. The first failure stops with its
# output; the last line is `ALL OK: ...` when everything passed. Like
# tsumugi's scripts/verify.sh.
#
# Clippy covers every OS minato ships for -- Linux, Windows and macOS -- from
# any machine: the host first, then the others. Those are type checks only --
# they never run that code, so read Windows path strings by eye. ring (rustls,
# under ureq) builds C for the target, so the other targets go through
# cargo-zigbuild, whose zig is a C compiler for all of them (installed from
# PyPI below when it is missing). zig has no MSVC headers, so off Windows the
# Windows clippy is x86_64-pc-windows-gnu; minato has no `target_env` code,
# so that is the same code CI's clippy-windows checks. Before 2026-10-16 the
# PC left Linux and macOS to CI and a cloud session left Windows.
# `cargo fmt --check` only looks; run `cargo fmt --all` to fix what it finds.

set -o pipefail
cd "$(dirname "$0")/.." || exit 1

host=$(rustc -vV | sed -n 's/^host: //p')
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

# cargo-zigbuild comes from PyPI with zig. Where pip may not touch the system
# Python (PEP 668) it goes to ~/.local/bin. zig's cache goes with Cargo's
# output (on the owner's PC the RAM disk, not the SSD).
zigbuild() {
    if ! command -v cargo-zigbuild >/dev/null 2>&1; then
        python3 -m pip install -q cargo-zigbuild >/dev/null 2>&1 \
            || python3 -m pip install -q --user --break-system-packages cargo-zigbuild >/dev/null 2>&1
        PATH="$PATH:$HOME/.local/bin"
        command -v cargo-zigbuild >/dev/null 2>&1 || { echo "FAILED: no cargo-zigbuild (pip install cargo-zigbuild)"; exit 1; }
    fi
    local dir
    dir=$(cargo metadata -q --no-deps --format-version 1 | sed 's/.*"target_directory":"\([^"]*\)".*/\1/; s/\\\\/\//g')
    run env RUSTUP_TOOLCHAIN=stable ZIG_GLOBAL_CACHE_DIR="$dir/zig" cargo-zigbuild "$@"
}

run cargo fmt --all --check
run cargo build -q --locked --workspace --all-features
run cargo test -q --workspace --all-features
tests=$(echo "$last" | grep 'test result' | awk '{p+=$4; f+=$6} END {print p " passed; " f " failed"}')
run cargo +stable clippy -q --workspace --all-targets --all-features -- -D warnings
for t in x86_64-unknown-linux-gnu x86_64-pc-windows-msvc aarch64-apple-darwin; do
    [ "$t" = "$host" ] && continue
    zig=1
    if [ "$t" = x86_64-pc-windows-msvc ]; then
        # An ARM64 Windows host builds ring's C with the MSVC tools.
        if [ -n "$windows" ]; then zig=; else t=x86_64-pc-windows-gnu; fi
    fi
    rustup target list --installed 2>/dev/null | grep -qx "$t" || rustup target add "$t" >/dev/null 2>&1
    if [ -n "$zig" ]; then
        zigbuild clippy -q --workspace --all-targets --all-features --target "$t" -- -D warnings
    else
        run cargo +stable clippy -q --workspace --all-targets --all-features --target "$t" -- -D warnings
    fi
done
run cargo +stable clippy -q --workspace --all-targets -- -D warnings
# The checklist follows TESTING.md.
run cargo run -q -p minato --example make-testcheck -- --check
echo "ALL OK: $tests"
