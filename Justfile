set shell := ["pwsh.exe", "-c"]

build:
    cargo build --release -p tauri-cli -j 12
    Copy-Item target\release\native.exe G:\Dx\bin\native.exe -Force
