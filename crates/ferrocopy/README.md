# FerroCopy (crate)

> **This is the crate-level README for the core engine crate.**
> For the full project documentation (Flutter Desktop GUI, Ratatui TUI, FFI bridge, etc.), see the [workspace root README](../../README.md).

[![Rust](https://img.shields.io/badge/rust-1.85+-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

The core Rust copy engine crate — parallel file copy, hash verification, shell integration, and CLI.
| 📎 Command Dispatcher | Type-safe `CopyCommand` dispatch (Start / Cancel / Pause / Resume / Retry) |
| 🔌 RPC Proxy Separation | Lapce-inspired stdio JSON-RPC split (`--use-rpc-proxy`) between CLI and engine process |
| ⚙️ Hot-Reload Config | Monitors `config.toml` via `notify-debouncer-full`; applies changes live |
| 🛑 Graceful Shutdown | Ctrl+C handler saves partial results to `shutdown_result.json` |

## Installation

### Build (Rust required)

```bash
git clone https://github.com/HHKK0127/FerroCopy.git
cd FerroCopy
cargo build --release
```

Build optimizations (pre-configured in `Cargo.toml`):
- `LTO = "fat"` — link-time optimization for size and speed
- `codegen-units = 1` — single codegen unit for max optimization
- `strip = "symbols"` — removes debug symbols, reduces binary size
- Minimal tokio feature set (`rt`, `io-util`, `sync`, `fs`, `time`, `macros`)

### Benchmarks (reference values)

Actual throughput depends on disk performance and file sizes.

| Scenario | Files | Total Size | Time | Avg Speed |
|----------|-------|-----------|------|-----------|
| Small (text/code) | 1,000 | ~50 MB | ~2-3 s | ~20 MB/s |
| Medium (images/PDF) | 100 | ~1 GB | ~8-12 s | ~100 MB/s |
| Large (video/ISO) | 10 | ~10 GB | ~20-30 s | ~400 MB/s |

*Note: Slower on HDD, faster on SSD.*

### Install Shell Integration

```bash
ferrocopy --install-context
```

Adds "Copy to FerroCopy" and "Move to FerroCopy" to the right-click menu.
"Paste to FerroCopy" is available from folder background right-click.

## Usage

### GUI Mode

```bash
ferrocopy --gui
```

### CLI Mode

```bash
ferrocopy /path/to/source /path/to/destination
```

### CLI Options

```bash
# 8-thread parallel copy + hash verification
ferrocopy /path/to/source /path/to/destination -j 8 --verify

# Move mode (delete originals after copy)
ferrocopy /path/to/source /path/to/destination --move-files

# Run copy through separated proxy process (stdio JSON-RPC)
ferrocopy /path/to/source /path/to/destination --use-rpc-proxy

# Install / uninstall shell integration
ferrocopy --install-context
ferrocopy --uninstall-context
```

## Architecture

```
ferrocopy/
├── src/
│   ├── main.rs      # CLI entry point + clap parser
│   ├── gui.rs       # egui-based GUI (dot design / ZED-inspired UI)
│   ├── engine.rs    # Parallel copy engine + error aggregation + backoff retry
│   ├── config.rs    # Configuration + TOML persistence + hot-reload
│   ├── hash.rs      # BLAKE3 / XXH3-128 verification
│   ├── dot.rs       # Dot-design components (batch-rendering optimised)
│   ├── plugin.rs    # Plugin trait + App builder + CopyCommand dispatcher
│   ├── signal.rs    # Ctrl+C handling + result persistence (graceful shutdown)
│   └── shell.rs     # Windows shell integration (registry / context menu)
├── Cargo.toml
└── README.md
```

### Data Flow

```
CLI args (clap)
    │
    ├── --install-context / --uninstall-context → shell.rs (registry ops)
    ├── --gui → gui.rs (eframe window)
    │              ├── dot.rs (particles, progress bars, buttons)
    │              ├── engine.rs (copy + retry + verify)
    │              ├── config.rs (TOML settings + hot-reload)
    │              ├── plugin.rs (Plugin/Command dispatch)
    │              └── shell.rs (context menu launch)
    └── (CLI) → engine.rs (parallel copy + error aggregation)
                  ├── signal.rs (Ctrl+C → save results)
                  └── config.rs (auto-reload settings)
```

### Error Handling (Lore-inspired)

```
CopySeverity: None < Skipped < Warning < Error < Fatal
CopyOutcome  : Per-file result (src, dst, severity, message, bytes)
EngineResult : Aggregated results (count, errors(), merge, Display)
```

- Copy continues even when errors occur (fatal errors also tolerated)
- All file results aggregated into `EngineResult`
- Check with `has_errors()`, iterate with `errors()`

### Retry Strategy (Warp-inspired)

```
RetryStrategy(initial_ms=100, max_ms=5000, max_retries=3, jitter_factor=0.3)
```

- Exponential backoff: 100 ms → 200 ms → 400 ms → ... → 5 s max
- Jitter (±30%): avoids thundering herd on concurrent retries
- Applied in `copy_file_with_move` / `remove_file_with_retry`

### Plugin Architecture (Bevy + Lapce inspired)

```rust
// Plugin trait — Bevy-style feature modularisation
pub trait Plugin {
    fn id(&self) -> PluginId;
    fn build(&self, app: &mut App);
}

// Command dispatcher — Lapce-style type-safe dispatch
pub enum CopyCommand {
    Start { sources: Vec<PathBuf>, destination: PathBuf },
    Cancel,
    Pause,
    Resume,
    Retry { file: PathBuf },
}
```

- `App::add_plugin()` to plug/unplug features
- `Dispatcher` executes `CopyCommand` variants
- CLI / GUI / Shell can be designed as independent plugins

### Config Hot-Reload (Alacritty-inspired)

- Settings persisted as TOML in `config.toml`
- File changes watched via `notify-debouncer-full`
- Auto-reload on change — no manual restart needed
- `FerroConfig` manages all settings centrally

### Graceful Shutdown (Yserver-inspired)

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
| Language | Rust (edition 2021) |
| Async Runtime | tokio (rt, io-util, sync, fs, time, macros) |
| GUI | egui 0.29 / eframe |
| Hashing | BLAKE3, xxhash-rust (XXH3-128) |
| Shell Integration | winreg 0.52, windows-rs 0.61 |
| Concurrency | tokio::sync::Semaphore |
| Timestamp Preservation | filetime 0.2 |
| File Discovery | walkdir 2 |
| Settings | serde + toml (persistence), notify-debouncer-full (hot-reload) |
| Signals | tokio::signal (Ctrl+C) |
| Random | fastrand (jitter retry) |

## License

MIT License

---

# FerroCopy 日本語

[![Rust](https://img.shields.io/badge/rust-1.85+-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

[TeraCopy](https://teracopy.en.softonic.com/) に触発された、Rust 製の高速ファイルコピーツールです。
並列コピーエンジン、ハッシュ検証、Windows シェル統合を備えています。
UI は「点（ドット）」をコンセプトにした宇宙テーマのパーティクルデザインを採用しています。

## 特徴

| 機能 | 説明 |
|------|------|
| ⚡ 並列コピー | マルチスレッドでファイルを同時コピー（`Semaphore` 制御） |
| 🔍 ハッシュ検証 | BLAKE3 / XXH3-128 で整合性確認 |
| 📂 再帰コピー | ディレクトリ構造を維持 |
| ⏯ 一時停止/再開 | コピー中に一時停止・再開可能 |
| 🪟 シェル統合 | 右クリックメニュー、Send To、ペースト |
| 🔁 Move モード | コピー後に元ファイルを削除（指数バックオフ + Jitter リトライ） |
| 🖥 GUI / CLI | 両方のインターフェースに対応 |
| 🌟 ドットデザインUI | 背景パーティクル、点の進捗バー、星型チェックボックス |
| 💪 エラー集約 | 全ファイルの結果を収集（致命的エラーでも継続） |
| 📏 アダプティブバッファ | 小ファイル=64KB / 大ファイル=1MB 自動切り替え |
| 🔄 指数バックオフ + Jitter | ファイルロック競合時に自動リトライ（初期 100ms・最大 5 秒） |
| 🧩 プラグインアーキテクチャ | Plugin trait + App builder で機能をモジュール化 |
| 📎 コマンドディスパッチャ | 型安全な CopyCommand ディスパッチ（Start/Cancel/Pause/Retry） |
| 🔌 RPC プロキシ分離 | Lapce 風 stdio JSON-RPC 分離（`--use-rpc-proxy`）で CLI とエンジンを別プロセス化 |
| ⚙️ 設定ホットリロード | TOML 設定ファイルを監視し、変更を自動検出・即時反映 |
| 🛑 Graceful Shutdown | Ctrl+C で安全終了、途中結果を JSON ファイルに保存 |

## インストール

### ビルド (Rust が必要)

```bash
git clone https://github.com/HHKK0127/FerroCopy.git
cd FerroCopy
cargo build --release
```

ビルド最適化（`Cargo.toml` に設定済み）:
- `LTO = "fat"` — リンク時最適化でバイナリサイズ・速度を改善
- `codegen-units = 1` — 単一コード生成ユニットで最大最適化
- `strip = "symbols"` — デバッグシンボルを除去しサイズ削減
- `tokio` 依存を最小限の機能セットに制限 (`rt`, `io-util`, `sync`, `fs`, `time`, `macros`)

### ベンチマーク（参考値）

以下の数値は開発環境での参考値です。実際の速度はディスク性能やファイルサイズに依存します。

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

### GUI モード

```bash
ferrocopy --gui
```

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

# シェル統合のインストール/アンインストール
ferrocopy --install-context
ferrocopy --uninstall-context
```

## アーキテクチャ

```
ferrocopy/
├── src/
│   ├── main.rs      # CLI エントリポイント + clap パーサー
│   ├── gui.rs       # egui ベースの GUI (ドットデザイン / ZED 風 UI)
│   ├── engine.rs    # 並列コピーエンジン + エラー集約 + 指数バックオフリトライ
│   ├── config.rs    # 設定と型定義 (TOML 永続化 + ホットリロード)
│   ├── hash.rs      # BLAKE3 / XXH3-128 検証
│   ├── dot.rs       # ドットデザイン部品 (一括描画最適化済み)
│   ├── plugin.rs    # Plugin trait + App builder + CopyCommand ディスパッチャ
│   ├── signal.rs    # Ctrl+C ハンドリング + 結果保存 (Graceful Shutdown)
│   └── shell.rs     # Windows シェル統合 (レジストリ / コンテキストメニュー)
├── Cargo.toml
└── README.md
```

### データフロー

```
CLI args (clap)
    │
    ├── --install-context / --uninstall-context → shell.rs (レジストリ操作)
    ├── --gui → gui.rs (eframe ウィンドウ)
    │              ├── dot.rs (パーティクル背景、進捗バー、ボタン)
    │              ├── engine.rs (並列コピー + リトライ + 検証)
    │              ├── config.rs (TOML 設定 + ホットリロード)
    │              ├── plugin.rs (Plugin/Command ディスパッチ)
    │              └── shell.rs (コンテキストメニューからの起動)
    └── (CLI) → engine.rs (並列コピー + エラー集約)
                  ├── signal.rs (Ctrl+C → 結果保存)
                  └── config.rs (設定自動リロード)
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

## 技術スタック

| コンポーネント | 技術 |
|---------------|------|
| 言語 | Rust (edition 2021) |
| 非同期ランタイム | tokio (rt, io-util, sync, fs, time, macros) |
| GUI | egui 0.29 / eframe |
| ハッシュ | BLAKE3, xxhash-rust (XXH3-128) |
| シェル統合 | winreg 0.52, windows-rs 0.61 |
| 並列実行 | tokio::sync::Semaphore |
| 検証 | filetime 0.2 (タイムスタンプ保存) |
| ファイル探索 | walkdir 2 |
| 設定 | serde + toml (永続化), notify-debouncer-full (ホットリロード) |
| シグナル | tokio::signal (Ctrl+C) |
| 乱数生成 | fastrand (Jitter リトライ用) |

## ライセンス

MIT License