@echo off
rem Windows twin of build/zig-cc.sh: zig as the C compiler for a cross target, for the
rem cross-platform lint run from the Windows PC that replaced the development Mac (5 Oct 2026).
rem cargo needs an executable here and cannot run a .sh, so this hands over to zig-cc.py.
rem Usage: set ZIG_TARGET=x86_64-linux-gnu and point CC_<target> and the linker at this file.
python "%~dp0zig-cc.py" %*
