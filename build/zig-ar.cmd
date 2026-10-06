@echo off
rem The archiver for the cross-platform lint on Windows: cc-rs needs an "ar" for the target, which
rem the Mac had built in and Windows does not. zig ships one. Set AR_<target> to this file.
zig ar %*
