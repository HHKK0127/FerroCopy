# FerroCopy

**高速ファイルコピーツール — Rust + egui + 先進モジュラーアーキテクチャ**

[![Rust](https://img.shields.io/badge/rust-1.85%2B-blue)](https://www.rust-lang.org)

> **TeraCopy インスパイア。Bevy / Lapce / Warp / WezTerm / Yserver / Alacritty の設計パターンを統合。**

## プロジェクト構造

├── Cargo.toml          # Workspace 設定
├── crates/
│   ├── ferrocopy/      # メインバイナリ + CLI + GUI
│   └── ferrocopy-hook/ # Windows シェル拡張 COM DLL
└── tests/              # 統合テスト

## 機能

| カテゴリ | 機能 | 状態 |
|----------|------|:----:|
| コアエンジン | 並列コピー / アダプティブバッファ / ディレクトリ再帰 / 3回リトライ | ✅ |
| GUI | egui/eframe / ZED風StatusBar / Toast / カラースキーム / taffy flexbox | ✅ |
| 検証 | BLAKE3 / XXH3-128 ハッシュ検証 | ✅ |
| リモート | SSH/SFTP (--ssh) / JSON-RPC (--rpc-listen) | ✅ |
| プラグイン | WASM (--wasm-plugin) / Luaフィルター (--lua-filter) | ✅ |
| シェル統合 | 右クリックメニュー / SendTo / ペースト / COM DLL | ✅ |
| モニタリング | Telemetry / Fanoutブロードキャスト / FileWatcher / シグナル | ✅ |
| ビルド | LTO / strip / 警告0 / 16.8MB リリース | ✅ |
| クラッシュレポート | パニックフック → ダンプ → MessageBox | ✅ |

## ビルド

`powershell
cargo build --release
ferrocopy --install-context   # シェル統合インストール
`

## CLI フラグ (v0.2)

--ssh, --ssh-key, --rpc-listen, --watch-dir, --lua-filter, --wasm-plugin, --use-schedule, --io-pool-threads

## ライセンス

MIT