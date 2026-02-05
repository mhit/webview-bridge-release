# WebView Bridge API ドキュメント

> 最終更新: 2026-02-05

## 概要

WebView Bridge は、WebView2 を使用したブラウザ自動化サーバーです。HTTP REST API を通じてブラウザセッションを制御できます。

## ベースURL

```
http://localhost:9400
```

---

## エンドポイント一覧

### ヘルスチェック

| エンドポイント | メソッド | 説明 |
|---------------|----------|------|
| `/health` | GET | サーバーの稼働状態を確認 |

**レスポンス:**
```
OK
```

---

### セッション管理

#### セッション作成

**POST** `/create`

```json
{
  "profile": "default",
  "headless": false,
  "user_agent": "Custom User Agent (optional)"
}
```

**レスポンス:**
```json
{
  "id": "abc12345-6789-...",
  "message": "Session created"
}
```

#### セッション削除

**DELETE** `/close/:id`

**レスポンス:**
```json
{
  "message": "Session closed"
}
```

#### セッション状態取得

**GET** `/status/:id`

**レスポンス:**
```json
{
  "id": "abc12345-6789-...",
  "status": "Ready",
  "url": "https://example.com",
  "options": {
    "profile": "default",
    "headless": false,
    "user_agent": null
  }
}
```

---

### ナビゲーション

#### ページ遷移

**POST** `/navigate/:id`

```json
{
  "url": "https://example.com"
}
```

**レスポンス:**
```json
{
  "success": true
}
```

---

### スクリプト実行

#### JavaScript実行

**POST** `/execute/:id`

```json
{
  "script": "return document.title"
}
```

**レスポンス:**
```json
{
  "result": "Example Domain"
}
```

---

### DOM操作

#### セレクター待機

**POST** `/wait/:id`

```json
{
  "selector": "#main-content",
  "timeout": 10000
}
```

**レスポンス:**
```json
{
  "found": true
}
```

#### 要素抽出

**POST** `/extract/:id`

```json
{
  "selector": "h1"
}
```

**レスポンス:**
```json
{
  "data": ["Example Domain"]
}
```

---

### スクリーンショット

**GET** `/screenshot/:id`

**レスポンス:**
```json
{
  "format": "png",
  "data": "iVBORw0KGgoAAAANSU..."
}
```

`data` は Base64 エンコードされた PNG 画像です。

---

### Cookie管理

#### Cookie取得

**GET** `/cookies/:id`

**レスポンス:**
```json
{
  "cookies": [
    {
      "name": "session_id",
      "value": "abc123",
      "domain": "example.com",
      "path": "/"
    }
  ]
}
```

#### Cookie設定

**POST** `/cookies/:id`

```json
{
  "cookies": [
    {
      "name": "session_id",
      "value": "abc123",
      "domain": "example.com",
      "path": "/",
      "secure": false,
      "httpOnly": false
    }
  ]
}
```

---

### Act API (OpenClaw互換)

#### クリック

**POST** `/act/:id`

```json
{
  "action": "click",
  "selector": "#submit-button"
}
```

#### テキスト入力

**POST** `/act/:id`

```json
{
  "action": "type",
  "selector": "#username",
  "value": "testuser"
}
```

#### キー押下

**POST** `/act/:id`

```json
{
  "action": "press",
  "key": "Enter"
}
```

---

### スナップショット

#### ページスナップショット取得

**GET** `/snapshot/:id`

**クエリパラメータ:**
- `format`: `html` | `text` | `aria` (default: `html`)

**レスポンス:**
```json
{
  "content": "<!DOCTYPE html>..."
}
```

---

## プロファイル管理

#### プロファイル一覧

**GET** `/profile/list`

**レスポンス:**
```json
{
  "profiles": ["default", "work", "personal"]
}
```

#### プロファイル作成

**POST** `/profile/create`

```json
{
  "name": "new_profile"
}
```

#### プロファイル削除

**DELETE** `/profile/:name`

---

## WebDriver Protocol (Selenium互換)

| エンドポイント | メソッド | 説明 |
|---------------|----------|------|
| `/wd/hub/status` | GET | WebDriver ステータス |
| `/wd/hub/session` | POST | セッション作成 |
| `/wd/hub/session/:id` | DELETE | セッション削除 |
| `/wd/hub/session/:id/url` | POST | ナビゲート |
| `/wd/hub/session/:id/url` | GET | 現在のURL取得 |
| `/wd/hub/session/:id/title` | GET | ページタイトル取得 |
| `/wd/hub/session/:id/source` | GET | ページソース取得 |
| `/wd/hub/session/:id/screenshot` | GET | スクリーンショット |
| `/wd/hub/session/:id/execute/sync` | POST | スクリプト実行 |
| `/wd/hub/session/:id/element` | POST | 要素検索 |
| `/wd/hub/session/:id/element/:eid/click` | POST | 要素クリック |
| `/wd/hub/session/:id/element/:eid/value` | POST | テキスト入力 |

---

## MCP (Model Context Protocol)

#### MCP情報取得

**GET** `/mcp/info`

**レスポンス:**
```json
{
  "name": "WebView Bridge",
  "version": "0.1.0",
  "capabilities": ["tools", "resources"]
}
```

#### ツール一覧

**GET** `/mcp/tools`

**レスポンス:**
```json
{
  "tools": [
    {
      "name": "navigate",
      "description": "Navigate to a URL",
      "inputSchema": {...}
    },
    ...
  ]
}
```

#### ツール実行

**POST** `/mcp/tools/call`

```json
{
  "name": "navigate",
  "arguments": {
    "url": "https://example.com"
  }
}
```

---

## CDP (Chrome DevTools Protocol)

| エンドポイント | メソッド | 説明 |
|---------------|----------|------|
| `/json/version` | GET | ブラウザバージョン情報 |
| `/json/list` または `/json` | GET | ターゲット一覧 |
| `/cdp/command` | POST | CDPコマンド実行 |

#### CDPコマンド例

**POST** `/cdp/command`

```json
{
  "sessionId": "abc123",
  "method": "Page.navigate",
  "params": {
    "url": "https://example.com"
  }
}
```

---

## エラーレスポンス

エラー時は適切なHTTPステータスコードと共にエラー情報が返されます：

```json
{
  "error": "Session not found"
}
```

| ステータスコード | 説明 |
|-----------------|------|
| 400 | リクエスト不正 |
| 404 | リソースが見つからない |
| 500 | サーバー内部エラー |
| 504 | タイムアウト |

---

## 使用例

### Python

```python
import requests

# セッション作成
session = requests.post("http://localhost:9400/create", 
    json={"profile": "default"}).json()
sid = session["id"]

# ページ遷移
requests.post(f"http://localhost:9400/navigate/{sid}", 
    json={"url": "https://example.com"})

# スクリプト実行
result = requests.post(f"http://localhost:9400/execute/{sid}", 
    json={"script": "return document.title"}).json()
print(result["result"])

# セッション終了
requests.delete(f"http://localhost:9400/close/{sid}")
```

### JavaScript

```javascript
// セッション作成
const session = await fetch("http://localhost:9400/create", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ profile: "default" })
}).then(r => r.json());

const sid = session.id;

// ページ遷移
await fetch(`http://localhost:9400/navigate/${sid}`, {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ url: "https://example.com" })
});

// スクリプト実行
const result = await fetch(`http://localhost:9400/execute/${sid}`, {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ script: "return document.title" })
}).then(r => r.json());

console.log(result.result);

// セッション終了
await fetch(`http://localhost:9400/close/${sid}`, { method: "DELETE" });
```

### PowerShell

```powershell
# セッション作成
$session = Invoke-RestMethod -Uri "http://localhost:9400/create" `
    -Method Post -Body '{"profile":"default"}' -ContentType "application/json"
$sid = $session.id

# ページ遷移
Invoke-RestMethod -Uri "http://localhost:9400/navigate/$sid" `
    -Method Post -Body '{"url":"https://example.com"}' -ContentType "application/json"

# スクリプト実行
$result = Invoke-RestMethod -Uri "http://localhost:9400/execute/$sid" `
    -Method Post -Body '{"script":"return document.title"}' -ContentType "application/json"
Write-Host $result.result

# セッション終了
Invoke-RestMethod -Uri "http://localhost:9400/close/$sid" -Method Delete
```
