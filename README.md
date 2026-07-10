# FerroCopy

FerroCopy は [TeraCopy](https://teracopy.en.softonic.com/) に触発された、Rust 製の高速ファイルコピーツールです。
並列コピーエンジン、ハッシュ検証、Windows シェル統合を備えています。

## 特徴

| 機能 | 説明 |
|------|------|
| ⚡ 並列コピー | マルチスレッドでファイルを同時コピー |
| 🔍 ハッシュ検証 | BLAKE3 / XXH3-128 で整合性確認 |
| 📂 再帰コピー | ディレクトリ構造を維持 |
| ⏯ 一時停止/再開 | コピー中に一時停止・再開可能 |
| 🪟 シェル統合 | 右クリックメニュー、Send To、ペースト |
| 🔁 Move モード | コピー後に元ファイルを削除 |
| 🖥 GUI / CLI | 両方のインターフェースに対応 |
| 🌟 ZED 風 UI | トースト通知、ステータスバー |

## インストール

### ビルド (Rust が必要)

```bash
git clone https://github.com/HHKK0127/FerroCopy.git
cd FerroCopy
cargo build --release
```

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
│   ├── main.rs      # CLI エントリポイント
│   ├── gui.rs       # egui ベースの GUI (ZED 風 UI)
│   ├── engine.rs    # 並列コピーエンジン (tokio + semaphore)
│   ├── config.rs    # 設定と型定義
│   ├── hash.rs      # BLAKE3 / XXH3-128 検証
│   └── shell.rs     # Windows シェル統合
├── Cargo.toml
└── README.md
```

## 技術スタック

- **言語**: Rust
- **ランタイム**: tokio (非同期 I/O)
- **GUI**: egui / eframe
- **ハッシュ**: BLAKE3, xxhash-rust
- **シェル統合**: winreg, windows-rs

## ライセンス

MIT License