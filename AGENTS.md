# WebView Bridge - エージェント連携ガイド

## 📋 プロジェクト概要

**WebView Bridge** は、Windows上で動作するWebView2ベースのWebスクレイピングサーバーです。OpenClawと連携し、安定したブラウザ自動化を提供します。

### 技術スタック

| カテゴリ | 技術選択 |
|---------|---------|
| 言語 | Rust |
| エディション | 2021 |
| HTTPサーバー | Axum 0.7 + Tokio |
| WebView2 | webview2-com 0.19.1 |
| シリアライゼーション | Serde + serde_json |
| トレーシング | tracing + tracing-subscriber |

---

## 🎯 開発モード: Solo

このプロジェクトは **Solo モード** で運用します。Claude Code が実装・テスト・検証を担当します。

### マーカー運用

Plans.md では `cc:*` マーカーを使用します：

| 操作 | マーカー | タイミング |
|------|----------|----------|
| タスク開始 | `cc:WIP` | 作業開始時 |
| タスク完了 | `cc:完了` | 完了時 |
| ブロッカー | `cc:ブロック中` | 何かを待つ場合 |

---

## 📖 開発フロー

### 基本パターン

```
1. 計画   → Plans.md にタスクを追加/確認
2. 実装   → /work または直接実装指示
3. 検証   → cargo build && cargo test
4. レビュー → コードレビューを実施
```

### 開発指示の例

| やりたいこと | 指示例 |
|-------------|--------|
| 続きをやる | 「続けて」「次のタスクを」 |
| 機能追加 | 「セッション一覧APIを追加して」 |
| バグ修正 | 「navigate APIがエラーになるのを直して」 |
| 検証 | 「ビルドして」「テスト実行して」 |
| 全部任せる | 「Plans.md のタスクを全部やって」 |

---

## 🔧 開発環境

### ビルド

```powershell
# Windows PowerShell
.\build.bat
```

または手動で:
```powershell
CARGO_INCREMENTAL=0 cargo build --release
```

### 実行

```powershell
.\run.bat
```

### テスト

```powershell
cargo test
```

### APIテスト

```powershell
.\test_api.ps1
```

---

## 📂 プロジェクト構造

```
webview-bridge/
├── src/
│   ├── main.rs              # エントリーポイント
│   ├── api.rs               # HTTP APIハンドラー
│   ├── core/
│   │   └── mod.rs           # コア機能
│   └── webview/
│       ├── mod.rs           # WebView モジュール
│       ├── webview_instance.rs
│       └── window.rs        # Win32 ウィンドウ作成
├── tests/
│   └── integration_test.rs  # 統合テスト
├── docs/
│   ├── design.md            # 設計書
│   └── use-cases.md         # ユースケース
└── Cargo.toml
```

---

## 🎯 現在のフェーズ

**Phase 1-2**: MVP実装中
- [x] Win32 ウィンドウ作成
- [x] SessionManager 実装
- [ ] WebView2 インスタンス管理
- [ ] HTTP API 実装
- [ ] 統合テスト

詳細は `Plans.md` を確認してください。

---

## 📝 依存関係の管理

新しい依存関係を追加する際は `Cargo.toml` を更新してください：

```toml
[dependencies]
crate-name = "version"
```

---

## 🔄 セッション継続

開発を再開する際は：
1. `/sync-status` で現在の状態を確認
2. `Plans.md` で次のタスクを確認（`cc:WIP` や `cc:TODO` のタスクを探す）
3. 「続けて」で実装を再開

### マーカー更新のタイミング

| 作業 | マーカー更新 |
|------|-------------|
| タスクを開始 | `cc:TODO` → `cc:WIP` |
| タスク完了 | `cc:WIP` → `cc:完了` |
| 何かを待つ | `cc:WIP` → `cc:ブロック中` |
