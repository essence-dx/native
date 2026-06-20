set shell := ["pwsh.exe", "-c"]

build:
    cargo build --release -p tauri-cli -j 12
    New-Item -ItemType Directory -Force -Path G:\Dx\bin | Out-Null
    Copy-Item target\release\native.exe G:\Dx\bin\native.exe -Force



