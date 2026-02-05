# WebView Bridge - Claude Code 設定

## 📋 プロジェクト固有ルール

このプロジェクトは **Rust で書かれた WebView2 ベースの Web スクレイピングサーバー** です。

---

## 🦀 Rust 開発規約

### 命名規則

| 種類 | 規則 | 例 |
|------|------|-----|
| 構造体 | PascalCase | `SessionManager`, `WebViewInstance` |
| 列挙型 | PascalCase | `SessionStatus`, `ApiError` |
| 関数/メソッド | snake_case | `create_session()`, `navigate_to()` |
| 定数 | SCREAMING_SNAKE_CASE | `MAX_SESSIONS`, `DEFAULT_PORT` |
| ローカル変数 | snake_case | `session_id`, `url` |
| ファイル名 | snake_case | `session_manager.rs`, `api_handler.rs` |

### コードスタイル

- **インデント**: 4スペース（Rust標準）
- **行幅**: 100文字（rustfmtデフォルト）
- **所有権**: 借用（`&`）を優先、所有権の移動は必要な場合のみ
- **エラー処理**: `Result<T, E>` と `?` 演算子を優先使用
- **非同期**: `async/await` を使用、`tokio` ランタイム

### テスト規約

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_creation() {
        // テスト実装
    }
}
```

---

## 🔧 開発ワークフロー

### 実装前

1. **Plans.md を確認**: 現在のタスクと優先度を確認
2. **関連コードを読む**: 変更箇所の周辺コードを理解
3. **テストを書く**: TDD原則、まずテストから

### 実装時

1. **cargo fmt**: コードフォーマットを維持
2. **cargo clippy**: リンター警告を修正
3. **所有権に注意**: 借用チェッカーと友達になる

### 実装後

1. **cargo test**: すべてのテストが通ることを確認
2. **cargo build --release**: リリースビルドが成功することを確認
3. **統合テスト**: APIの動作確認

---

## 🚫 禁止事項

- **`unwrap()` の濫用**: 本番コードでは `Result` を適切に処理
- **`panic!` の使用**: エラーは `Result` で返す
- **テストの skip**: 失敗したテストは修正する
- **ハードコードされた値**: 定数または設定ファイルを使用

---

## 📦 依存関係の追加

新しいクレートを追加する際は：

1. **バージョン互換性を確認**: `Cargo.toml` の既存依存関係と競合しないこと
2. **ライセンスを確認**: プロジェクトに適していること（MIT互換）
3. **メンテナンス状態**: 活発にメンテナンスされているクレートを選択

```toml
# 例
[dependencies]
new-crate = "0.x"
```

---

## 🎯 プロジェクト特有の注意点

### WebView2 関連

- Windows API は **Windowsのみ** で動作
- `webview2-com` のバージョンは `0.19.1` で固定
- `windows` クレートのバージョンは `0.39.0` で統一

### API 設計

- OpenClaw との互換性を維持
- REST API は JSON で通信
- エラーレスポンスは一貫した形式

---

## 📝 このファイルの目的

このファイルは Claude Code のプロジェクト固有の設定と規約を定義します。
グローバル設定（`~/.claude/CLAUDE.md`）を上書きします。

変更はプロジェクトの進化に合わせて更新してください。
