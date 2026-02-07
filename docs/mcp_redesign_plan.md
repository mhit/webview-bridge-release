# MCP AIフレンドリー再設計計画書 v2

## 概要

WebView Bridge MCPレイヤーをAIフレンドリーに再設計する。
V2 REST APIは温存し、MCPレイヤーのみを抽象化。

### 設計原則
1. **V2 REST APIは変更なし**（プログラマブル用途）
2. **シンプルな6ツール構成**（32個→6個）
3. **外部AIが自律ループを回す**（内部AI不要）
4. **各ツールは単一責務**

---

## アーキテクチャ

```
┌─────────────────────────────────────────────────────────────┐
│                  External AI (Claude/Gemini)                 │
│                                                              │
│                 ┌──────────────────────────┐                 │
│                 │   AI Autonomous Loop     │                 │
│                 │   1. capture → 状態確認  │                 │
│                 │   2. 判断 → 次のアクション│                 │
│                 │   3. interact → 実行     │                 │
│                 │   4. 繰り返し            │                 │
│                 └──────────────────────────┘                 │
└─────────────────────────────────────────────────────────────┘
                              │
                             MCP (6 tools)
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                     WebView Bridge                           │
│                                                              │
│   ┌─────────────────────────────────────────────────────┐   │
│   │  navigate  │  interact  │  capture                  │   │
│   │  extract   │  session   │  execute                  │   │
│   └─────────────────────────────────────────────────────┘   │
│                              │                               │
│                              ↓                               │
│                        V2 REST API                           │
└─────────────────────────────────────────────────────────────┘
```

---

## 新MCP構成（6ツール）

| ツール | 用途 | V2マッピング |
|--------|------|--------------|
| **navigate** | URL遷移 | POST /navigate |
| **interact** | 操作マクロ | POST /click, /type, /execute |
| **capture** | 状態取得 | POST /screenshot, /execute |
| **extract** | データ抽出 | POST /execute, /ai/extract |
| **session** | セッション管理 | POST /session/* |
| **execute** | カスタムJS | POST /execute |

---

## ツール詳細設計

### 1. `navigate` - ページ遷移

URLを指定してページを開く。

```json
{
  "url": "https://example.com",
  "wait_for": "load",
  "timeout_ms": 30000
}
```

| パラメータ | 型 | 必須 | 説明 |
|-----------|------|------|------|
| url | string | ✅ | 遷移先URL |
| wait_for | string | - | `load`, `networkidle`, `selector` |
| wait_selector | string | - | wait_for=selectorの場合 |
| timeout_ms | number | - | タイムアウト（default: 30000） |

**レスポンス**:
```json
{
  "success": true,
  "url": "https://example.com/redirected",
  "title": "Example Page"
}
```

---

### 2. `interact` - 操作マクロ

複数の操作を一括実行。`actions`配列で指定。

```json
{
  "actions": [
    {"type": "type", "target": "#email", "value": "user@example.com"},
    {"type": "type", "target": "#password", "value": "secret"},
    {"type": "click", "target": "button[type=submit]"},
    {"type": "wait", "for": "url_contains", "value": "/dashboard"}
  ]
}
```

| パラメータ | 型 | 必須 | 説明 |
|-----------|------|------|------|
| actions | array | ✅ | アクション配列 |

**サポートするアクション**:

| type | パラメータ | 説明 |
|------|-----------|------|
| `click` | target | 要素クリック |
| `type` | target, value, clear | テキスト入力 |
| `scroll` | direction, amount | スクロール |
| `hover` | target | ホバー |
| `select` | target, value | セレクト選択 |
| `press` | key, modifiers | キー入力 |
| `wait` | for, value, timeout_ms | 条件待機 |

**target指定方法**:
- CSS: `"#login-btn"`, `".submit"`
- XPath: `"xpath://button[@type='submit']"`
- テキスト: `"text:ログイン"`

**レスポンス**:
```json
{
  "success": true,
  "results": [
    {"action": "type", "success": true},
    {"action": "type", "success": true},
    {"action": "click", "success": true},
    {"action": "wait", "success": true}
  ]
}
```

---

### 3. `capture` - 状態取得

ページの状態を取得。デフォルトで主要情報を返す。

```json
{}
```

デフォルトレスポンス:
```json
{
  "url": "https://example.com/page",
  "title": "Page Title",
  "screenshot": "base64...",
  "text": "ページのテキスト内容..."
}
```

オプション指定:

```json
{
  "only": ["screenshot"],
  "selector": "#main-content",
  "full_page": true
}
```

| パラメータ | 型 | 必須 | 説明 |
|-----------|------|------|------|
| only | array | - | 取得項目を限定（`screenshot`, `text`, `html`, `cookies`, `images`） |
| selector | string | - | 特定領域のみ |
| full_page | boolean | - | フルページスクリーンショット |

---

### 4. `extract` - データ抽出

CSSセレクタまたはAIでデータを構造化抽出。

**CSSセレクタモード**:
```json
{
  "selector": ".product-item",
  "fields": {
    "name": "h2",
    "price": ".price",
    "url": "a@href",
    "image": "img@src"
  },
  "limit": 20
}
```

**AI抽出モード**（セレクタ不明時）:
```json
{
  "goal": "商品名と価格を抽出",
  "schema": {
    "type": "array",
    "items": {
      "type": "object",
      "properties": {
        "name": {"type": "string"},
        "price": {"type": "number"}
      }
    }
  }
}
```

| パラメータ | 型 | 必須 | 説明 |
|-----------|------|------|------|
| selector | string | △ | CSSセレクタ（CSS抽出時） |
| fields | object | △ | フィールドマッピング（CSS抽出時） |
| goal | string | △ | 抽出目標（AI抽出時） |
| schema | object | - | 出力スキーマ（AI抽出時） |
| limit | number | - | 最大件数 |

**レスポンス**:
```json
{
  "success": true,
  "data": [
    {"name": "商品A", "price": 1980, "url": "...", "image": "..."},
    {"name": "商品B", "price": 2480, "url": "...", "image": "..."}
  ],
  "count": 2
}
```

---

### 5. `session` - セッション管理

セッションの取得・解放・一覧。パラメータで操作を指定。

**取得**:
```json
{
  "acquire": "my-session",
  "headless": false,
  "restore": true
}
```

**解放**:
```json
{
  "release": "my-session"
}
```

**一覧**:
```json
{
  "list": true
}
```

| パラメータ | 型 | 必須 | 説明 |
|-----------|------|------|------|
| acquire | string | △ | 取得するセッション名 |
| release | string | △ | 解放するセッション名 |
| list | boolean | △ | セッション一覧を取得 |
| headless | boolean | - | ヘッドレスモード（acquire時） |
| restore | boolean | - | 前回のURLを復元（acquire時、default: true） |

**レスポンス（acquire）**:
```json
{
  "session": "my-session",
  "is_new": false,
  "restored_url": "https://example.com/last-page"
}
```

**レスポンス（list）**:
```json
{
  "sessions": [
    {"name": "my-session", "active": true, "last_url": "..."},
    {"name": "other", "active": false, "last_url": "..."}
  ]
}
```

---

### 6. `execute` - カスタムJS

任意のJavaScriptを実行。低レベル脱出口。

```json
{
  "script": "document.querySelectorAll('.item').length"
}
```

| パラメータ | 型 | 必須 | 説明 |
|-----------|------|------|------|
| script | string | ✅ | 実行するJavaScript |

**レスポンス**:
```json
{
  "success": true,
  "result": 42
}
```

---

## セッションの暗黙的な使用

すべてのツール（`session`以外）は暗黙的に現在のセッションを使用。

**明示的なセッション指定**（オプション）:
```json
{
  "session": "specific-session",
  "url": "https://example.com"
}
```

**セッション未取得時の動作**:
- 自動的に`default`セッションを取得

---

## 使用例

### ログインフロー

```
1. session.acquire("rakuten")
2. navigate("https://login.rakuten.co.jp")
3. capture() → ログインフォームを確認
4. interact([
     {type: "type", target: "#email", value: "..."},
     {type: "type", target: "#password", value: "..."},
     {type: "click", target: "#login-btn"}
   ])
5. capture() → ログイン成功を確認
6. session.release("rakuten")
```

### 商品情報抽出

```
1. navigate("https://search.rakuten.co.jp/search/mall/keyword")
2. extract({
     selector: ".searchresultitem",
     fields: {name: ".title", price: ".price", url: "a@href"}
   })
3. → 構造化データ取得
```

### AI抽出（セレクタ不明時）

```
1. navigate("https://unknown-site.com/products")
2. extract({
     goal: "商品名、価格、在庫状況を抽出",
     schema: {...}
   })
3. → AIが自動でセレクタを推論して抽出
```

---

## V2 API マッピング

| MCP Tool | V2 API |
|----------|--------|
| navigate | POST /navigate |
| interact.click | POST /click |
| interact.type | POST /type |
| interact.wait | POST /wait |
| interact.scroll | POST /execute (JS) |
| capture.screenshot | POST /screenshot |
| capture.text | POST /execute (innerText) |
| capture.cookies | GET /cookies |
| extract.css | POST /execute (querySelectorAll + map) |
| extract.ai | POST /ai/extract |
| session.acquire | POST /session/acquire |
| session.release | POST /session/release |
| session.list | GET /session/list |
| execute | POST /execute |

---

## 実装計画

### フェーズ1: ツール定義（1-2時間）
1. `src/mcp/mod.rs` 作成
2. 6ツールのスキーマ定義
3. 型定義（Request/Response）

### フェーズ2: ツール実装（3-4時間）
1. `navigate` - 既存navigate呼び出し
2. `interact` - アクションループ実装
3. `capture` - 複合レスポンス構築
4. `extract` - CSS抽出 + AI抽出分岐
5. `session` - 既存session関数呼び出し
6. `execute` - 既存execute呼び出し

### フェーズ3: MCPサーバー（2時間）
1. stdio/HTTP両対応のMCPプロトコル実装
2. JSON-RPC 2.0ハンドリング
3. ツールディスパッチャー

### フェーズ4: テスト（1時間）
1. 各ツールの単体テスト
2. 実際のAI（Claude）からの呼び出しテスト

---

## 設計上のポイント

### なぜ内部AIを廃止したか

1. **二重構造の複雑さ**: 外部AI → 内部AI は責任が曖昧
2. **外部AIの能力**: Claude/Geminiは自分でループを回せる
3. **デバッグ困難**: 内部AIの判断が見えにくい
4. **シンプルさ**: 6ツールで十分な表現力

### `capture`の重要性

AIが状態を理解するための主要ツール。
スクリーンショット + テキストで現在の状態を把握し、次のアクションを決定。

```
loop:
  state = capture()
  if goal_achieved(state): break
  next_action = decide(state)
  interact(next_action)
```

### `extract`のAIモード

セレクタが分からない未知のサイトで使用。
内部でGemini Visionを呼び出してセレクタを推論。
※ 要: Gemini APIキー設定
