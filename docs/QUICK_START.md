# WebView Bridge クイックスタート

5分でWebView Bridgeを使い始める！

## 前提条件

- Windows 10/11
- Rust (cargo) インストール済み
- AIプロバイダー設定（Ollama or Gemini API）

## Step 1: ビルド＆起動（2分）

```bash
# リポジトリクローン
git clone https://github.com/user/webview-bridge
cd webview-bridge

# リリースビルド
cargo build --release

# 起動
cargo run --release
```

起動成功時の出力:
```
Loaded config from: C:\Users\{user}\.webview-bridge\config.toml
Config loaded: AI enabled=true, provider=ollama, model=gpt-oss:20b
[SessionManagerV2] Loaded 0 sessions from disk
```

## Step 2: 設定確認（1分）

`~/.webview-bridge/config.toml` を編集:

```toml
[server]
host = "127.0.0.1"
port = 3030

[ai]
enabled = true
provider = "ollama"     # Ollama使用時
model = "gpt-oss:20b"   # お使いのモデル

# Gemini使用時
# provider = "gemini"
# model = "gemini-2.0-flash"
# api_key = "your-api-key"
```

## Step 3: 動作確認（2分）

### REST API テスト

```powershell
# ヘルスチェック
Invoke-RestMethod http://localhost:3030/health
# → "OK"

# MCPツール一覧
Invoke-RestMethod http://localhost:3030/mcp/tools
```

### MCP経由で操作

AIエージェント（Claude Desktop、OpenClaw等）から:

```
DuckDuckGoを開いてRustを検索して
```

裏で実行されるMCPコール:
```json
// 1. セッション作成
{"tool": "session", "arguments": {"acquire": "default"}}

// 2. ページ遷移
{"tool": "navigate", "arguments": {"url": "https://duckduckgo.com"}}

// 3. 検索
{"tool": "interact", "arguments": {
  "actions": [
    {"type": "type", "target": "#searchbox_input", "value": "Rust"},
    {"type": "click", "target": "button[type=submit]"}
  ]
}}

// 4. 結果確認
{"tool": "capture", "arguments": {"summarize": true}}
```

## よく使うパターン

### Bot対策サイト（Amazon等）

```json
{
  "tool": "interact",
  "arguments": {
    "actions": [
      {"type": "type", "target": "#twotabsearchtextbox", "value": "Anker", "instant": true}
    ],
    "options": {
      "human_mode": true
    }
  }
}
```

**ポイント:**
- `human_mode: true` → 人間らしい動き（ベジェ曲線、タイポ等）
- `instant: true` → サジェスト回避

### 自律型Agent

```json
{
  "tool": "agent",
  "arguments": {
    "action": {
      "type": "start",
      "goal": "Amazonでワイヤレスマウスを検索して価格順に並べる",
      "human_mode": true,
      "instant_type": true,
      "max_steps": 10
    }
  }
}
```

→ AIが自動でページ分析、クリック、入力を繰り返して目標達成！

### データ抽出

```json
{
  "tool": "extract",
  "arguments": {
    "selector": ".product-card",
    "fields": {
      "title": ".title",
      "price": ".price",
      "link": "a@href"
    },
    "limit": 20
  }
}
```

→ 構造化されたJSONで商品情報を取得

## トラブルシューティング

### Q: 「Session not found」エラー

```json
{"tool": "session", "arguments": {"acquire": "default"}}
```
でセッションを作成してから操作

### Q: クリックが効かない

1. `capture` でスクリーンショット確認
2. セレクタが正しいか確認
3. `human_mode: true` で試す

### Q: タイプが途中で切れる

`instant: true` を追加（サジェスト干渉回避）

### Q: Bot検出される

1. `headless: false` でウィンドウ表示
2. `human_mode: true` で人間らしい動き
3. 直接URL遷移を避け、クリックで遷移

## 次のステップ

- [MCP_TOOLS.md](./MCP_TOOLS.md) - 全ツールの詳細
- [ARCHITECTURE.md](./ARCHITECTURE.md) - 内部構造
- [API.md](./API.md) - REST API リファレンス

---

**Happy Automating! 🚀**
