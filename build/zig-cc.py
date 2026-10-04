"""zig as the C compiler for a cross target: the logic of build/zig-cc.sh, for Windows.

cc-rs adds its own `--target=<triple>`; zig cc wants `-target <triple>` and rejects having both,
so the wrapper drops cc-rs's flag and puts zig's in. Called through build/zig-cc.cmd.
"""
import os
import subprocess
import sys

target = os.environ.get('ZIG_TARGET')
if not target:
    sys.exit('zig-cc: set ZIG_TARGET, e.g. x86_64-linux-gnu')
args = [a for a in sys.argv[1:] if not a.startswith('--target=')]
sys.exit(subprocess.call(['zig', 'cc', '-target', target] + args))
