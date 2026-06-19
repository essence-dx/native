// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use serde_json::{json, Value};

pub(super) fn representative_project_config_fixture() -> Value {
  json!({
    "identifier": "com.dx.representative",
    "productName": "DX Representative",
    "version": "2.31.0",
    "build": {
      "beforeDevCommand": "bun run dev",
      "beforeBuildCommand": "bun run build",
      "devUrl": "http://localhost:3000",
      "frontendDist": "../out"
    },
    "app": {
      "windows": representative_window_fixtures("main", 72),
      "security": {
        "csp": "default-src 'self'; img-src 'self' asset: https://asset.localhost; connect-src ipc: http://ipc.localhost https://api.localhost; style-src 'self' 'unsafe-inline'",
        "assetProtocol": {
          "enable": true,
          "scope": representative_scope_fixtures("asset", 96)
        }
      },
      "trayIcon": {
        "iconPath": "icons/tray.png",
        "iconAsTemplate": false
      }
    },
    "bundle": {
      "active": true,
      "targets": ["msi", "nsis", "app", "dmg", "deb", "rpm"],
      "resources": representative_resource_fixtures("resource", 192),
      "externalBin": representative_resource_fixtures("external-bin", 32),
      "windows": {
        "wix": {
          "language": representative_language_fixtures(24),
          "fragmentPaths": representative_resource_fixtures("wix-fragment", 32)
        },
        "nsis": {
          "installMode": "both",
          "languages": representative_language_fixtures(18),
          "template": "packaging/windows/installer-template.nsi"
        }
      }
    },
    "plugins": {
      "fs": {
        "scope": representative_scope_fixtures("fs", 260)
      },
      "shell": {
        "open": true,
        "scope": representative_scope_fixtures("shell", 140)
      },
      "http": {
        "scope": representative_url_fixtures(96)
      },
      "dx": {
        "datasets": representative_dataset_fixtures("dataset", 180)
      }
    }
  })
}

pub(super) fn representative_platform_config_fixture() -> Value {
  json!({
    "productName": "DX Representative Platform",
    "app": {
      "windows": representative_window_fixtures("platform", 24),
      "security": {
        "assetProtocol": {
          "enable": true,
          "scope": representative_scope_fixtures("platform-asset", 48)
        }
      }
    },
    "bundle": {
      "resources": representative_resource_fixtures("platform-resource", 48),
      "windows": {
        "webviewInstallMode": {
          "type": "downloadBootstrapper"
        }
      }
    },
    "plugins": {
      "fs": {
        "scope": representative_scope_fixtures("platform-fs", 80)
      },
      "dx": {
        "datasets": representative_dataset_fixtures("platform-dataset", 40)
      }
    }
  })
}

fn representative_window_fixtures(prefix: &str, count: usize) -> Vec<Value> {
  (0..count)
    .map(|index| {
      json!({
        "label": format!("{prefix}-window-{index:03}"),
        "title": format!("DX Representative {prefix} Window {index:03}"),
        "url": format!("/workspaces/{prefix}/lanes/{index:03}/overview?receipt=machine-cache"),
        "width": 960 + (index % 5) * 32,
        "height": 640 + (index % 7) * 24,
        "minWidth": 720,
        "minHeight": 480,
        "resizable": index % 3 != 0,
        "fullscreen": false,
        "decorations": true,
        "visible": index < 8,
        "center": index % 2 == 0
      })
    })
    .collect()
}

fn representative_scope_fixtures(prefix: &str, count: usize) -> Vec<String> {
  (0..count)
    .map(|index| format!("$APPDATA/dx/{prefix}/workspace-{index:03}/receipts/**/*.json"))
    .collect()
}

fn representative_resource_fixtures(prefix: &str, count: usize) -> Vec<String> {
  (0..count)
    .map(|index| {
      format!(
        "fixtures/{prefix}/workspace-{index:03}/machine-cache/receipt-artifact-{index:03}.json"
      )
    })
    .collect()
}

fn representative_url_fixtures(count: usize) -> Vec<String> {
  (0..count)
    .map(|index| format!("https://api-{index:03}.dx.localhost/v1/machine-cache/**"))
    .collect()
}

fn representative_language_fixtures(count: usize) -> Vec<String> {
  const LANGUAGES: &[&str] = &[
    "en-US", "fr-FR", "de-DE", "es-ES", "it-IT", "ja-JP", "ko-KR", "pt-BR", "ru-RU", "zh-CN",
    "zh-TW", "nl-NL",
  ];
  (0..count)
    .map(|index| LANGUAGES[index % LANGUAGES.len()].to_string())
    .collect()
}

fn representative_dataset_fixtures(prefix: &str, count: usize) -> Vec<Value> {
  (0..count)
    .map(|index| {
      json!({
        "name": format!("{prefix}-{index:03}"),
        "source": format!("fixtures/{prefix}/source/config-{index:03}.json"),
        "machine": format!(".dx/tauri/{prefix}-{index:03}.machine"),
        "schema": "dx.tauri.representative.fixture",
        "readMode": if index % 2 == 0 { "mmap" } else { "bounded-read" },
        "fingerprint": {
          "blake3": format!("fixture-blake3-{prefix}-{index:03}"),
          "bytes": 4096 + index * 137
        },
        "paths": representative_scope_fixtures(prefix, 3)
      })
    })
    .collect()
}
