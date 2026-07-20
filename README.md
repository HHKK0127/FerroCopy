# FerroCopy

[![Rust](https://img.shields.io/badge/rust-1.85+-orange.svg)](https://www.rust-lang.org)
[![Flutter](https://img.shields.io/badge/flutter-3.29-blue.svg)](https://flutter.dev)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A high-speed file copy tool for Windows, inspired by [TeraCopy](https://teracopy.en.softonic.com/).  
Built with a **Rust copy engine** at its core, available via **Flutter Desktop GUI** and **Ratatui TUI**.

> **Hybrid architecture**: Rust handles all file I/O; Flutter and TUI are independent frontends.

---

[EN] · [JP](#ferrocopy-日本語)

---

## Features

| Feature | Description |
|---------|-------------|
| ⚡ Parallel Copy | Multi-threaded concurrent file copy (Semaphore controlled) |
| 🔍 Hash Verification | Integrity check via BLAKE3 / XXH3-128 |
| 📂 Recursive Copy | Preserves full directory structure |
| ⏯ Pause / Resume | Suspend and resume copy operations |
| 🔁 Move Mode | Copy + delete originals (exponential backoff + jitter retry) |
| 🪟 Shell Integration | Right-click menu, Send To, Paste (Phase 1), COM DLL (Phase 2) |
| 🖥 Flutter Desktop UI | Material 3 + Tokyo Night theme, TeraCopy-inspired layout |
| 🖥 Ratatui TUI | Terminal UI with 7-panel layout, keyboard-driven |
| 💪 Error Aggregation | Collects all results; continues on non-fatal errors |
| 📏 Adaptive Buffer | 64 KB for small files, 1 MB for large files, auto-switch |
| 🔄 Exponential Backoff + Jitter | Auto-retry on lock contention (100 ms initial, 5 s max) |
| 🧩 Plugin Architecture | Plugin trait + App builder for modular features |
| 📎 Command Dispatcher | Type-safe CopyCommand dispatch (Start / Cancel / Pause / Resume / Retry) |
| 🔌 RPC Proxy Separation | stdio JSON-RPC split between CLI and engine process |
| ⚙️ Hot-Reload Config | Monitors config.toml via notify-debouncer-full |
| 🛑 Graceful Shutdown | Ctrl+C handler saves partial results |
| 🔁 Event Loop | Non-blocking recv_timeout-based event loop |
| 📋 Clipboard | Copy/paste paths via arboard |
| 🎨 Color Schemes | 5 presets (Deep Space, Midnight Nebula, Solar Flare, Aurora, Blood Moon) |
| 🧵 IoTaskPool | 4-worker dedicated I/O thread pool |
| 📡 Telemetry | 1-second diagnostic loop (files/sec, bytes/sec, error rate) |
| 🚀 SSH/SFTP | Remote copy via libssh-rs (user@host, custom port, key auth) |
| 📅 Stage Pipeline | Bevy-style Stage/System pipeline for structured copy workflows |
| 🖥 Event-Driven | CoreSender/CoreReceiver for real-time copy event streaming |
| 🔗 FFI Bridge | 5 extern "C" functions for Flutter ↔ Rust integration |
| 🎯 Extended CLI | --ssh, --rpc-listen, --watch-dir, --lua-filter, --wasm-plugin, --use-schedule |

## Architecture

```
ferrocopy (workspace — 6 crates)
├── ferrocopy (lib+bin)         ← Rust copy engine (23 modules)
│   ├── src/lib.rs              → Public library (config/engine/hash)
│   └── src/main.rs             → CLI + 23 module integration
├── crates/engine-ffi           → cdylib: extern "C" × 5
├── crates/tui-test             → Ratatui TUI (7-panel layout)
├── crates/ferrocopy-hook       → COM DLL shell extension
├── crates/quadcomp             → wgpu GPU render engine
└── assets/ico_builder          → Icon builder

ferrocopy_desktop (Flutter Desktop — 11 files)
├── lib/main.dart               → Entry point
├── lib/theme/tokyo_night.dart  → Material 3 dark theme
├── lib/screens/main_screen.dart → Main layout (TeraCopy-style)
├── lib/models/
│   ├── app_state.dart          → State management + FFI calls
│   └── copy_item.dart          → File copy item model
├── lib/widgets/
│   ├── toolbar.dart            → Toolbar (Source/Dest/Actions)
│   ├── file_list.dart          → File table
│   ├── progress_bar.dart       → Progress + statistics
│   ├── log_panel.dart          → Log display
│   └── settings_panel.dart     → Settings panel
└── lib/ffi/engine_ffi.dart     → FFI bindings (DynamicLibrary)
```

### Data Flow

```
┌──────────────┐     ┌───────────────┐     ┌──────────────┐
│  Flutter UI  │◄───►│  engine_ffi   │◄───►│  ferrocopy   │
│  (Material)  │ FFI │  DLL (cdylib) │ ABI │  (lib+bin)   │
└──────────────┘     └───────────────┘     └──────┬───────┘
       ▲                                          │
       │                                          ▼
┌──────┴──────┐                          ┌───────────────┐
│  Ratatui    │◄─────────────────────────┤  SSH/SFTP     │
│  TUI (term) │  direct Rust call        │  RPC / Watch  │
└─────────────┘                          └───────────────┘
```

## Installation

### Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | 1.85+ | Engine build |
| Flutter | 3.29.2+ | Desktop UI build |
| Visual Studio | 17+ (18 patched) | Windows native build |

### Build All

```bash
# Clone
git clone https://github.com/HHKK0127/FerroCopy.git
cd FerroCopy

# Rust crates
cargo build --release -p ferrocopy
cargo build --release -p engine-ffi
cargo build --release -p tui-test

# Flutter Desktop
cd ferrocopy_desktop
flutter pub get
flutter build windows --debug
cd ..
```

Build optimizations (pre-configured):
- `LTO = "fat"` — link-time optimization
- `codegen-units = 1` — single codegen unit
- `strip = "symbols"` — removes debug symbols
- Minimal tokio feature set (`rt`, `io-util`, `sync`, `fs`, `time`, `macros`)

### DLL Deployment

For Flutter to call the Rust engine, copy `engine_ffi.dll` to the build output:

```bash
copy target\release\engine_ffi.dll ferrocopy_desktop\build\windows\x64\runner\Debug\
```

### Install Shell Integration (Rust CLI)

```bash
ferrocopy --install-context
```

Adds "Copy to FerroCopy" and "Move to FerroCopy" to the right-click menu.

## Usage

### Flutter Desktop GUI

```bash
cd ferrocopy_desktop
flutter run -d windows
```

### Ratatui TUI (Terminal)

```bash
cargo run --release -p tui-test
```

Keyboard shortcuts:
| Key | Action |
|-----|--------|
| `e` | Edit source path |
| `d` | Edit destination path |
| `Enter` | Start copy |
| `Space` | Pause / Resume |
| `Esc` | Cancel |
| `q` | Quit |

### CLI Mode (Rust)

```bash
# Basic copy
ferrocopy /path/to/source /path/to/destination

# 8-thread parallel + hash verification
ferrocopy /path/to/source /path/to/destination -j 8 --verify

# Move mode
ferrocopy /path/to/source /path/to/destination --move-files

# RPC proxy mode
ferrocopy /path/to/source /path/to/destination --use-rpc-proxy

# SSH remote
ferrocopy user@host:/remote/path /local/destination

# Lua filter
ferrocopy /src /dst --lua-filter myfilter.lua

# WASM plugin
ferrocopy /src /dst --wasm-plugin myplugin.wasm

# Schedule pipeline
ferrocopy /src /dst --use-schedule

# Install / uninstall shell integration
ferrocopy --install-context
ferrocopy --uninstall-context
```

### Error Handling (Lore-inspired)

```
CopySeverity: None < Skipped < Warning < Error < Fatal
CopyOutcome  : Per-file result (src, dst, severity, message, bytes)
EngineResult : Aggregated results (count, errors(), merge, Display)
```

- Copy continues even when errors occur (fatal errors also tolerated)
- All file results aggregated into `EngineResult`
- `run_copy_engine_with_events()` streams real-time `CopyEvent` via `CoreSender`

### Retry Strategy (Warp-inspired)

```
RetryStrategy(initial_ms=100, max_ms=5000, max_retries=3, jitter_factor=0.3)
```

- Exponential backoff: 100 ms → 200 ms → 400 ms → ... → 5 s max
- Jitter (±30%): avoids thundering herd on concurrent retries
- Applied in `copy_file_with_move` / `remove_file_with_retry`

### Plugin Architecture (Bevy + Lapce inspired)

- `Plugin` trait with `id()` and `build()` for modular features
- `CopyCommand` enum: Start / Cancel / Pause / Resume / Retry
- CLI / GUI / Shell can be designed as independent plugins

### Event Loop

- Non-blocking `recv_timeout(16ms)` for ~60 FPS polling
- `EventHandler` trait for custom callbacks (on_start, on_progress, etc.)
- `LogHandler` — default stdout logging implementation

### Config Hot-Reload (Alacritty-inspired)

- TOML settings monitored via `notify-debouncer-full`
- Auto-reload on change — no manual restart needed

### Stage Pipeline (Bevy-inspired)

```
Stage 1: Scan → Stage 2: Copy → Stage 3: Verify → Stage 4: Cleanup
```

- `Schedule::run()` sequential execution
- `Schedule::cancel()` stops mid-pipeline

### IoTaskPool

- 4 dedicated worker threads for blocking I/O
- Prevents GUI freezes during large file operations

### SSH/SFTP Remote Copy

- `libssh-rs` based SFTP file transfer
- Supports custom port and key-based authentication

```rust
pub struct Shutdown {
    pub activated: bool,
    pub completed_files: Vec<CopyOutcome>,
    pub partial_results: bool,
}
```

- Ctrl+C detected → safely stops in-flight tasks
- Completed file results saved to `shutdown_result.json`
- Resume state can be inspected on next launch

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Language | Rust (edition 2021), Dart 3.7 |
| Async Runtime | tokio (rt, io-util, sync, fs, time, macros) |
| GUI (Desktop) | Flutter / Material 3 |
| GUI (Terminal) | Ratatui 0.29 + Crossterm |
| Hashing | BLAKE3, xxhash-rust (XXH3-128) |
| Shell Integration | winreg 0.52, windows-rs 0.58 |
| FFI Bridge | dart:ffi (cdylib, extern "C") |
| State Management | ChangeNotifier + Provider |
| Concurrency | tokio::sync::Semaphore |
| I/O Pool | crossbeam-channel, thread_local |
| Event Channel | crossbeam-channel (CoreSender) |
| SSH | libssh-rs |
| RPC | tokio + serde_json (TCP) |
| Hot-Reload | notify-debouncer-full |
| Lua | mlua |
| WASM | wasmtime |
| GPU | wgpu (quadcomp) |

## Project History

FerroCopy evolved through 4 major phases:

| Phase | Scope | Outcome |
|:---|---:|:---|
| **Rust Engine** | Parallel copy, hash verify, 23 modules, CLI | Core engine + all features |
| **Shell Integration** | Context menu, COM DLL Phase 2 | Windows integration |
| **Flutter Desktop** | Material 3 UI, FFI bridge, Tokyo Night | Modern desktop GUI |
| **Ratatui TUI** | 7-panel terminal UI, keyboard-driven | Terminal alternative |

## License

MIT License

---

# FerroCopy 日本語

[![Rust](https://img.shields.io/badge/rust-1.85+-orange.svg)](https://www.rust-lang.org)
[![Flutter](https://img.shields.io/badge/flutter-3.29-blue.svg)](https://flutter.dev)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

[TeraCopy](https://teracopy.en.softonic.com/) に触発された、Rust 製の高速ファイルコピーツールです。
**Rust コピーエンジン** を核とし、**Flutter Desktop GUI** と **Ratatui TUI** の 2 つの UI から操作可能なハイブリッド構成です。

> **ハイブリッドアーキテクチャ**: Rust がファイル I/O を担当し、Flutter と TUI は独立したフロントエンドとして動作します。

## 特徴

| 機能 | 説明 |
|------|------|
| ⚡ 並列コピー | マルチスレッドでファイルを同時コピー（`Semaphore` 制御） |
| 🔍 ハッシュ検証 | BLAKE3 / XXH3-128 で整合性確認 |
| 📂 再帰コピー | ディレクトリ構造を維持 |
| ⏯ 一時停止/再開 | コピー中に一時停止・再開可能 |
| 🔁 Move モード | コピー後に元ファイルを削除（指数バックオフ + Jitter リトライ） |
| 🪟 シェル統合 | 右クリックメニュー、Send To、COM DLL（Phase 2 完了） |
| 🖥 Flutter Desktop UI | Material 3 + Tokyo Night テーマ、TeraCopy 風レイアウト |
| 🖥 Ratatui TUI | 7分割レイアウトのターミナル UI、キーボード操作対応 |
| 💪 エラー集約 | CopySeverity/CopyOutcome/EngineResult で全結果を収集（致命的エラーでも継続） |
| 📏 アダプティブバッファ | 小ファイル=64KB / 大ファイル=1MB 自動切り替え |
| 🔄 指数バックオフ + Jitter | ファイルロック競合時に自動リトライ（初期 100ms・最大 5 秒） |
| 🧩 プラグインアーキテクチャ | Plugin trait + App builder で機能をモジュール化 |
| 📎 コマンドディスパッチャ | 型安全な CopyCommand ディスパッチ（Start/Cancel/Pause/Retry） |
| 🔌 RPC プロキシ分離 | Lapce 風 stdio JSON-RPC 分離（`--use-rpc-proxy`）で CLI とエンジンを別プロセス化 |
| ⚙️ 設定ホットリロード | TOML 設定ファイルを監視し、変更を自動検出・即時反映 |
| 🛑 Graceful Shutdown | Ctrl+C で安全終了、途中結果を JSON ファイルに保存 |
| 🔁 イベントループ | recv_timeout ベースの非ブロッキングイベントループ |
| 📋 クリップボード | arboard 経由でパスをコピー/ペースト |
| 🎨 カラースキーム | 5プリセット（Deep Space / Midnight Nebula / Solar Flare / Aurora / Blood Moon） |
| 🧵 IoTaskPool | 4ワーカー専用I/Oスレッドプール |
| 📡 テレメトリ | 1秒診断ループ（files/sec / bytes/sec / error rate） |
| 🚀 SSH/SFTP | libssh-rs ベースのリモートコピー（user@host / カスタムポート / 鍵認証） |
| 📅 ステージパイプライン | Bevy 風 Stage/System で構造化コピーワークフロー |
| 🖥 イベント駆動 | CoreSender / CoreReceiver でリアルタイムコピーイベント配信 |
| 🔗 FFI ブリッジ | 5 extern "C" 関数で Flutter ↔ Rust 連携 |
| 🎯 拡張 CLI | --ssh, --rpc-listen, --watch-dir, --lua-filter, --wasm-plugin, --use-schedule |

## インストール

### 前提条件

| ツール | バージョン | 用途 |
|--------|-----------|------|
| Rust | 1.85+ | エンジンビルド |
| Flutter | 3.29.2+ | デスクトップ UI ビルド |
| Visual Studio | 2022+ (CMake 対応) | Flutter デスクトップビルド |

### ビルド手順

```bash
# 1. リポジトリをクローン
git clone https://github.com/HHKK0127/FerroCopy.git
cd FerroCopy

# 2. Rust エンジンをビルド
cargo build --release

# 3. FFI ブリッジ DLL をビルド（Flutter 向け）
cd crates/engine-ffi
cargo build --release
cd ../..

# 4. DLL を Flutter プロジェクトにコピー
cp target/release/engine_ffi.dll ferrocopy_desktop/

# 5. Flutter デスクトップ UI をビルド
cd ferrocopy_desktop
flutter build windows --release
```

### ビルド最適化（`Cargo.toml` に設定済み）
- `LTO = "fat"` — リンク時最適化でバイナリサイズ・速度を改善
- `codegen-units = 1` — 単一コード生成ユニットで最大最適化
- `strip = "symbols"` — デバッグシンボルを除去しサイズ削減
- `tokio` 依存を最小限の機能セットに制限 (`rt`, `io-util`, `sync`, `fs`, `time`, `macros`)

### ベンチマーク（参考値）

| シナリオ | ファイル数 | 総サイズ | 経過時間 | 平均速度 |
|---------|-----------|---------|---------|---------|
| 小ファイル（テキスト/コード） | 1,000 | 約 50 MB | 約 2-3 秒 | 〜20 MB/s |
| 中ファイル（画像/PDF） | 100 | 約 1 GB | 約 8-12 秒 | 〜100 MB/s |
| 大ファイル（動画/ISO） | 10 | 約 10 GB | 約 20-30 秒 | 〜400 MB/s |

*注: HDD 環境では速度が低下し、SSD 環境ではさらに高速になります。*

### シェル統合をインストール

```bash
ferrocopy --install-context
```

右クリックメニューに「FerroCopyにコピー」「FerroCopyに移動」が追加されます。
「FerroCopyにペースト」はフォルダ背景の右クリックから使用できます。

## 使い方

### Flutter Desktop GUI モード

```bash
cd ferrocopy_desktop
flutter run --debug
# またはビルド済みバイナリを直接実行
./build/windows/runner/Release/ferrocopy_desktop.exe
```

ファイル選択 → コピー/移動 → 進捗確認 までの一連の操作を GUI で行えます。

### Ratatui TUI モード

```bash
cargo run --release -p tui-test
```

7分割レイアウトのターミナル UI。キーボードショートカット:
| キー | 操作 |
|------|------|
| `Tab` | パネル切替 |
| `↑/↓` | ファイル選択 |
| `Enter` | コピー開始 |
| `Space` | 一時停止/再開 |
| `Esc` | キャンセル |
| `q` | 終了 |

### CLI モード

```bash
ferrocopy /path/to/source /path/to/destination
```

### CLI オプション

```bash
# 8スレッドで並列コピー + ハッシュ検証
ferrocopy /path/to/source /path/to/destination -j 8 --verify

# Move モード (コピー後に削除)
ferrocopy /path/to/source /path/to/destination --move-files

# 分離プロキシ経由で実行 (stdio JSON-RPC)
ferrocopy /path/to/source /path/to/destination --use-rpc-proxy

# SSH リモートコピー
ferrocopy user@host:/remote/path ./local/dst --ssh-key ~/.ssh/id_rsa

# ディレクトリ監視
ferrocopy --watch-dir ./watch --dest ./output

# Lua フィルター
ferrocopy ./src ./dst --lua-filter ./filter.lua

# WASM プラグイン
ferrocopy ./src ./dst --wasm-plugin ./plugin.wasm

# シェル統合のインストール/アンインストール
ferrocopy --install-context
ferrocopy --uninstall-context
```

## アーキテクチャ

```
ferrocopy (workspace — 6 crates)
├── ferrocopy (lib+bin)         ← Rust コピーエンジン (23 モジュール)
│   ├── src/lib.rs              → 公開ライブラリ (config/engine/hash)
│   └── src/main.rs             → CLI + 23 モジュール統合エントリポイント
├── crates/engine-ffi           → cdylib: extern "C" × 5
├── crates/tui-test             → ⭐ Ratatui TUI (7分割レイアウト)
├── crates/ferrocopy-hook       → COM DLL シェル拡張
├── crates/quadcomp             → wgpu GPU 描画エンジン
└── assets/ico_builder          → アイコンビルドツール

ferrocopy_desktop (Flutter Desktop — 11 UI ファイル)
├── lib/main.dart               → エントリポイント
├── lib/theme/tokyo_night.dart  → Material 3 + Tokyo Night ダークテーマ
├── lib/screens/main_screen.dart → メインレイアウト (TeraCopy 風)
├── lib/models/
│   ├── app_state.dart          → 全状態管理 (ChangeNotifier + FFI 呼び出し)
│   └── copy_item.dart          → ファイルコピーアイテムモデル
├── lib/widgets/
│   ├── toolbar.dart            → ツールバー (Source/Dest/アクション)
│   ├── file_list.dart          → ファイルテーブル
│   ├── progress_bar.dart       → 進捗バー + 統計情報
│   ├── log_panel.dart          → ログ表示
│   └── settings_panel.dart     → 設定パネル
└── lib/ffi/engine_ffi.dart     → FFI バインディング (DynamicLibrary)
```

### データフロー

```
┌──────────────┐     ┌───────────────┐     ┌──────────────┐
│  Flutter UI  │◄───►│  engine_ffi   │◄───►│  ferrocopy   │
│  (Material)  │ FFI │  DLL (cdylib) │ ABI │  (lib+bin)   │
└──────────────┘     └───────────────┘     └──────┬───────┘
       ▲                                          │
       │                                          ▼
┌──────┴──────┐                          ┌───────────────┐
│  Ratatui    │◄─────────────────────────┤  SSH/SFTP     │
│  TUI (term) │  direct Rust call        │  RPC / Watch  │
└─────────────┘                          └───────────────┘
```

### エラー処理設計 (Lore インスパイア)

```
CopySeverity: None < Skipped < Warning < Error < Fatal
CopyOutcome  : 各ファイルの結果 (src, dst, severity, message, bytes)
EngineResult : 全結果を集約 (count, errors(), merge, Display)
```

- エラーが発生してもコピーは継続（致命的なエラーでも続行）
- 全ファイルの結果を `EngineResult` に集約
- `has_errors()` でエラーの有無を確認、`errors()` でイテレーション
- `run_copy_engine_with_events()` で CoreSender 経由のリアルタイム CopyEvent ストリーミング

### リトライ戦略 (Warp インスパイア)

```
RetryStrategy(initial_ms=100, max_ms=5000, max_retries=3, jitter_factor=0.3)
```

- 指数バックオフ: 100ms → 200ms → 400ms → ... → 最大 5 秒
- Jitter (±30%): 同時リトライの衝突を回避
- `copy_file_with_move` / `remove_file_with_retry` で適用

### プラグインアーキテクチャ (Bevy + Lapce インスパイア)

```rust
// Plugin trait — Bevy 風の機能モジュール化
pub trait Plugin {
    fn id(&self) -> PluginId;
    fn build(&self, app: &mut App);
}

// Command dispatcher — Lapce 風の型安全ディスパッチ
pub enum CopyCommand {
    Start { sources: Vec<PathBuf>, destination: PathBuf },
    Cancel,
    Pause,
    Resume,
    Retry { file: PathBuf },
}
```

- `App::add_plugin()` で機能を着脱
- `Dispatcher` が `CopyCommand` を実行
- CLI/GUI/Shell を独立したプラグインとして設計可能

### 設定ホットリロード (Alacritty インスパイア)

- TOML 形式で設定を `config.toml` に永続化
- `notify-debouncer-full` でファイル変更を監視
- 変更検出時に自動リロード（手動再起動不要）
- `FerroConfig` が全設定を一元管理

### イベントループ (Bevy インスパイア)

```
EventLoop<EventHandler> — 非ブロッキング tick() またはブロッキング run()
  ├── recv_timeout(16ms) — 約60FPSでイベントポーリング
  ├── on_start / on_progress / on_complete / on_error コールバック
  └── LogHandler — デフォルトの標準出力ログ実装
```

- `EventLoop::run()` — CLI モード用ブロッキングループ
- `EventLoop::tick()` — GUI モード用非ブロッキング単発ポーリング
- `EventHandler` トレイトでカスタムイベントコールバック

### Graceful Shutdown (Yserver インスパイア)

```rust
pub struct Shutdown {
    pub activated: bool,
    pub completed_files: Vec<CopyOutcome>,
    pub partial_results: bool,
}
```

- Ctrl+C 検出 → 進行中タスクを安全に停止
- 完了済みファイルの結果を `shutdown_result.json` に保存
- 次回起動時に中断状態を確認可能

### イベント駆動通信

```rust
// CoreSender — tokio タスク間で共有可能なクローン可能イベント送信
let (tx, rx) = CoreSender::<CopyEvent>::new();
let sender = tx.clone();
tokio::spawn(async move { sender.send(CopyEvent::FileCompleted { ... }); });

// CoreReceiver — ブロッキング / 非ブロッキング受信
while let Some(event) = rx.recv() {
    match event {
        CopyEvent::FileCompleted { file, bytes, .. } => { /* UI更新 */ }
        CopyEvent::Error { file, error, .. } => { /* トースト表示 */ }
        CopyEvent::Finished { .. } => { /* 完了 */ }
    }
}
```

- `CoreSender` は `Clone` — tokio タスク間で共有可能
- `CoreReceiver` は `recv()`（ブロッキング）と `try_recv()`（非ブロッキング）対応
- `run_copy_engine_with_events()` でリアルタイム UI 更新に使用

## 技術スタック

| コンポーネント | 技術 |
|---------------|------|
| 言語 | Rust (edition 2021), Dart 3.7+ |
| 非同期ランタイム | tokio (rt, io-util, sync, fs, time, macros) |
| GUI (Desktop) | Flutter / Material 3 + Tokyo Night |
| GUI (Terminal) | Ratatui (7分割レイアウト) |
| FFI ブリッジ | dart:ffi (DynamicLibrary, extern "C") |
| 状態管理 | ChangeNotifier + Provider |
| ハッシュ | BLAKE3, xxhash-rust (XXH3-128) |
| シェル統合 | winreg 0.52, windows-rs 0.58 |
| 並列実行 | tokio::sync::Semaphore |
| ファイル探索 | walkdir 2 |
| 設定 | serde + toml, notify-debouncer-full |
| シグナル | tokio::signal (Ctrl+C) |
| SSH | libssh-rs |
| RPC | tokio + serde_json (TCP) |
| ホットリロード | notify-debouncer-full |
| Lua | mlua |
| WASM | wasmtime |
| GPU | wgpu (quadcomp) |
| I/O プール | crossbeam-channel, thread_local |
| イベントチャネル | crossbeam-channel (CoreSender/CoreReceiver) |

## プロジェクト履歴

FerroCopy は 4 つの主要フェーズを経て進化しました:

| フェーズ | 範囲 | 成果 |
|:---|---:|:---|
| **Rust Engine** | 並列コピー、ハッシュ検証、23 モジュール、CLI | コアエンジン完成 |
| **Shell Integration** | コンテキストメニュー、COM DLL Phase 2 | Windows 統合 |
| **Flutter Desktop** | Material 3 UI、FFI ブリッジ、Tokyo Night | モダンデスクトップ GUI |
| **Ratatui TUI** | 7分割ターミナル UI、キーボード操作 | ターミナル代替 UI |

## ライセンス

MIT License
