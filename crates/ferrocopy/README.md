# FerroCopy

FerroCopy は [TeraCopy](https://teracopy.en.softonic.com/) に触発された、Rust 製の高速ファイルコピーツールです。
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
| 🔁 Move モード | コピー後に元ファイルを削除（3回リトライ + バックオフ） |
| 🖥 GUI / CLI | 両方のインターフェースに対応 |
| 🌟 ドットデザインUI | 背景パーティクル、点の進捗バー、星型チェックボックス |
| 💪 エラー集約 | 全ファイルの結果を収集（致命的エラーでも継続） |
| 📏 アダプティブバッファ | 小ファイル=64KB / 大ファイル=1MB 自動切り替え |

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
│   ├── engine.rs    # 並列コピーエンジン + エラー集約 (CopySeverity/CopyOutcome)
│   ├── config.rs    # 設定と型定義
│   ├── hash.rs      # BLAKE3 / XXH3-128 検証
│   ├── dot.rs       # ドットデザイン部品 (パーティクル、進捗バー、ボタン等)
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
    │              ├── engine.rs (並列コピー)
    │              └── shell.rs (コンテキストメニューからの起動)
    └── (CLI) → engine.rs (並列コピー + エラー集約)
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

## ライセンス

MIT License