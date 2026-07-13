# FerroCopy Project Plan

## 全体進捗: **100%** 🎉 (69 tests passing, 0 compile errors)

## Workspace 構成

| クレート | パス | 説明 |
|---------|------|------|
| `ferrocopy` | `crates/ferrocopy/` | メインCLI + GUI ファイルコピーツール |
| `ferrocopy-hook` | `crates/ferrocopy-hook/` | Windows シェル拡張 COM DLL |
| `yserver-engine` | `crates/yserver-engine/` | wgpu ベース GPU 描画エンジン (Phase 1) |

## 分析ベース機能候補 (全Phase統合)

| 優先度 | 機能 | ファイル | 参照元 | 完了 | 備考 |
|:------:|------|---------|--------|:----:|------|
| P0 | 指数バックオフ+Jitter | `engine.rs` | Warp | ✅ | |
| P0 | プラグインアーキテクチャ | `plugin.rs` | Bevy | ✅ | Plugin trait + Registry |
| P0 | コマンドディスパッチャ | `plugin.rs` | Lapce | ✅ | CopyCommand ディスパッチ |
| P0 | プロキシ/RPC分離 | `rpc.rs`, `main.rs` | Lapce | ✅ | |
| P0 | 設定ホットリロード | `config.rs` | Alacritty/Bevy | ✅ | |
| P0 | IoTaskPool | `iopool.rs` | Bevy | ✅ | 4-worker スレッドプール |
| P0 | LoopTelemetry | `telemetry.rs` | Yserver | ✅ | 1秒診断ループ |
| P0 | SSH/SFTP リモートコピー | `ssh.rs` | WezTerm | ✅ | libssh2 認証+転送 |
| P1 | RPCメッセージプロトコル | `rpc.rs` | Lapce | ✅ | JSON-RPC request/response |
| P1 | シグナルハンドリング | `signal.rs` | Yserver/Alacritty | ✅ | ctrl-c グレースフルシャットダウン |
| P1 | CoreSender/Waker イベント駆動 | `events.rs`, `eventloop.rs` | Yserver | ✅ | crossbeam_channel + EventLoop |
| P1 | Schedule/System パイプライン | `schedule.rs` | Bevy | ✅ | Scan→Copy→Verify→Report |
| P1 | イベントループ+Polling | `eventloop.rs` | Lapce | ✅ | 16ms tick + recv_timeout |
| P2 | クリップボード統合 | `clipboard.rs` | Warp | ✅ | arboard パス取得+履歴 |
| P2 | カラースキーム切替 | `color_scheme.rs` | WezTerm | ✅ | 5テーマ+egui適用 |
| P2 | チェンジディテクション | `change_detection.rs` | Bevy | ✅ | 状態変更時のみ UI 再描画 |
| P2 | ThreadBound Executor | `threadbound.rs` | Bevy | ✅ | UIスレッド安全な非同期 |
| P3 | taffy flexbox | `flexbox.rs` | Rio | ✅ | Flexboxリサイズ → GUI連携済み |
| P3 | CrashReporter | `crash_reporter.rs` | — | ✅ | panicフック+クラッシュダンプ+MsgBox |
| P4 | Lua拡張 | `lua_ext.rs` | WezTerm | ✅ | mlua 5.4 フィルタリング |
| P4 | fanout 進捗同報 | `fanout.rs` | Yserver | ✅ | 進捗同報+統合テスト |

### yserver-engine (GPU 描画エンジン)

| 優先度 | 機能 | ファイル | 完了 | 備考 |
|:------:|------|---------|:----:|------|
| P1 | メッセージプロトコル | `engine/message.rs` | ✅ | Create/Update/RemoveLayer, Resize, Shutdown, Pointer/KeyEvent |
| P1 | レイヤー状態管理 | `engine/state.rs` | ✅ | HashMap + z-sorting + dirty rect マージ |
| P1 | コンポジター | `engine/compositor.rs` | ✅ | z-order 収集 + ダメージ解析 |
| P1 | コアループ | `engine/core_loop.rs` | ✅ | Drain → Compose → Render → Telemetry |
| P1 | テレメトリー | `engine/telemetry.rs` | ✅ | 1秒ローリング FPS/タイミング診断 |
| P1 | wgpu レンダリング | `render/wgpu_backend.rs` | ✅ | 四辺形描画、スコープレクト部分更新 |
| P1 | WGSL シェーダー | `shaders.wgsl` | ✅ | 頂点+フラグメントパイプライン |
| P1 | winit デモ | `main.rs` | ✅ | 3層デモウインドウ |
| P2 | 入力抽象化 | `input/mod.rs` | ✅ | winit イベント→Message 変換、キーコード→文字 |
| P2 | プラットフォーム抽象化 | `platform/` | ✅ | Win/Linux/macOS 情報 |

## 残タスク ❌

全機能完了。将来の拡張候補:

| 優先度 | 機能 | 備考 |
|:------:|------|------|
| P4 | Lua拡張高度機能 | 基本filter関数はCLI統合済み |
| P4 | fanout GUI購読 | engine_worker内でbroadcast呼び出し済み (GUI未購読) |
| 保留 | Sentry本格導入 | 現状はCrashReporterで代替中、別クレート化検討 |
| 保留 | yserver-engine Phase 2 | テクスチャ/フォントレンダリング、アニメーション |

## ビルド

```bash
cargo build --release           # target/release/ferrocopy.exe + yserver-engine example
cargo test --workspace          # 69 tests: ferrocopy 65 + yserver-engine 3 + hook 1
cargo test -p yserver-engine    # 3 tests (lib only)
```
