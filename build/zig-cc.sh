#!/bin/sh
# zig as the C compiler for a cross target, used by the cross-platform lint.
#
# Why this exists: the development machine is a Mac, so `cargo clippy` only ever
# looks at the macOS branches of every `#[cfg(target_os = ...)]`. The Windows and
# Linux branches - which is where the system DNS, the service and the firewall live -
# were never linted locally, and the first public CI run failed on six lints nobody
# could have seen here (16 sep 2026). Now they can be linted before pushing.
#
# cc-rs adds its own `--target=<triple>`; zig cc wants `-target <triple>` and rejects
# having both, so the wrapper drops cc-rs's flag and puts zig's in.
#
# Usage: ZIG_TARGET=x86_64-linux-gnu build/zig-cc.sh <the rest of the cc arguments>
: "${ZIG_TARGET:?set ZIG_TARGET, e.g. x86_64-linux-gnu or x86_64-windows-gnu}"
args=""
for a in "$@"; do
  case "$a" in --target=*) ;; *) args="$args '$a'" ;; esac
done
eval exec zig cc -target "$ZIG_TARGET" $args
