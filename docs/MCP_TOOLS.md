# WebView Bridge MCP Tools

Claude/OpenClaw等のAIがブラウザを操作するためのMCPサーバー。

## 🔧 Available Tools

### 1. **session** - セッション管理
```json
{
  "acquire": "my-session",    // セッション取得（なければ作成）
  "headless": false,          // false=ウィンドウ表示（Bot対策サイト推奨）
  "release": "my-session",    // セッション解放
  "list": true                // 全セッション一覧
}
```

### 2. **navigate** - ページ遷移
```json
{
  "session": "default",
  "url": "https://example.com",
  "wait_for": "stable",       // "load" | "stable" | "networkidle" | "selector"
  "wait_selector": "#content" // wait_for=selectorの場合
}
```

### 3. **capture** - ページ状態取得
```json
{
  "session": "default",
  "screenshot": true,         // スクリーンショット保存
  "summarize": true,          // **NEW** AIでページを要約
  "include": ["full_text", "cookies", "html", "images"],
  "full_page": true           // 全ページキャプチャ
}
```

### 4. **interact** - ブラウザ操作
```json
{
  "session": "default",
  "actions": [
    {"type": "click", "target": "#button"},
    {"type": "type", "target": "#input", "value": "text", "clear": true},
    {"type": "scroll", "direction": "down", "amount": 500},
    {"type": "wait", "condition": "element", "value": "#loaded"}
  ],
  "options": {
    "human_mode": true        // Bot対策：人間らしい動き
  }
}
```

### 5. **extract** - 構造化データ抽出
```json
{
  "session": "default",
  "selector": ".product-card",
  "fields": {
    "title": ".title",
    "price": ".price",
    "link": "a@href"          // @attrで属性値取得
  },
  "limit": 10,
  "scroll_for_more": true     // 無限スクロール対応
}
```

### 6. **execute** - JavaScript実行
```json
{
  "session": "default",
  "script": "document.title"
}
```

### 7. **agent** - 🆕 自律型ブラウザ操作
```json
{
  "session": "default",
  "action": {
    "type": "start",
    "goal": "DuckDuckGoで「Rust」を検索する",
    "max_steps": 5,
    "system_prompt": "価格を重視して判断してください"  // カスタム指示
  }
}
```

**エージェントが自動で:**
- ページ状態を分析
- 次のアクションを決定（click/type/navigate）
- 目標達成まで繰り返し

---

## 🧪 OpenClawでのテスト

### テスト1: 基本的なナビゲーション
```
セッションを作成してGoogleを開き、ページタイトルを確認して
```

### テスト2: AI要約機能（summarize）
```
Wikipediaの「Rust (programming language)」ページを開いて、要約して
capture(summarize=true) を使用
```

### テスト3: 自律型エージェント
```
DuckDuckGoを開いて、agentツールで「WebView2」を検索させて
agent(goal="検索ボックスにWebView2と入力して検索する")
```

### テスト4: カスタムプロンプト付きエージェント
```
Amazonを開いて、agentで「ワイヤレスマウス」を検索
system_prompt="価格の安い順にソートしてください" を追加
```

### テスト5: データ抽出
```
ニュースサイトを開いて、見出しとリンクを抽出して
extract(selector=".headline", fields={title: "h2", link: "a@href"})
```

---

## ⚠️ Bot対策サイトのコツ

1. **headless: false** - ウィンドウ表示モード
2. **human_mode: true** - 人間らしい操作
3. **直接URL回避** - ホームページ→クリックで遷移
4. **適度な待機** - wait actionを挟む

---

## 📍 設定

AI設定（config.toml）:
```toml
[ai]
enabled = true
provider = "ollama"           # または "gemini"
model = "gpt-oss:20b"
# api_key = "..."             # Geminiの場合
```
