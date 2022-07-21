@echo off
::set RUSTFLAGS=-C target-feature=+crt-static
::#cargo +nightly build --release
cargo build --release
echo f | xcopy /y ".\target\i686-pc-windows-msvc\release\bhoptimer_helper.dll" "D:\steamcmd\cstrike\cstrike\addons\sourcemod\extensions\bhoptimer_helper.ext.dll"
