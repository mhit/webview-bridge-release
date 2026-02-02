# WebView Bridge 設計書

## 1. プロジェクト概要

### 1.1 背景と問題提起

現在のOpenClawにおけるWeb操作には以下の課題がある：

| 方式 | 問題点 |
|------|--------|
| **Puppeteer/Chrome** | WSL2環境での不安定さ、ゾンビプロセス蓄積、`--single-process`問題、メモリリーク |
| **Browser Relay** | ユーザーが手動でタブをアタッチ必要、常時ブラウザ起動が前提 |
| **HttpClient直接** | Cloudflare/Bot検出でブロック、JavaScript実行不可 |
| **Selenium/WebDriver** | 重い、ChromeDriverバージョン管理が煩雑 |

### 1.2 解決策

**WebView2** をベースとしたWindowsネイティブサーバーアプリケーションを構築し、OpenClawからHTTP/WebSocket経由で操作可能にする。

### 1.3 なぜWebView2か

- ✅ Windows 10/11に標準搭載（Edge Runtime）
- ✅ Cloudflare/Bot検出を自然にバイパス（通常ブラウザとして認識）
- ✅ 安定したAPI（Microsoft公式サポート）
- ✅ ヘッドレス動作可能
- ✅ Cookie/セッション管理が容易
- ✅ Claude Usage Monitorで実証済み

---

## 2. アーキテクチャ

### 2.1 全体構成

```
┌─────────────────────────────────────────────────────────────┐
│                        WSL2 (Linux)                          │
│  ┌─────────────┐     ┌─────────────┐     ┌──────────────┐  │
│  │  OpenClaw   │────▶│   Agent     │────▶│  browser     │  │
│  │  Gateway    │     │  (Claude)   │     │  tool call   │  │
│  └─────────────┘     └─────────────┘     └──────┬───────┘  │
│                                                   │          │
└───────────────────────────────────────────────────┼──────────┘
                                                    │ HTTP/WS
                                                    ▼
┌─────────────────────────────────────────────────────────────┐
│                      Windows Host                            │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              WebView Bridge Server                    │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────────────┐   │   │
│  │  │ HTTP API │  │ Session  │  │  WebView2 Pool   │   │   │
│  │  │ Server   │  │ Manager  │  │  ┌────┐ ┌────┐  │   │   │
│  │  └────┬─────┘  └────┬─────┘  │  │ WV1│ │ WV2│  │   │   │
│  │       │             │        │  └────┘ └────┘  │   │   │
│  │       └─────────────┴────────┴────────┬────────┘   │   │
│  └───────────────────────────────────────┼────────────┘   │
│                                          │                 │
│                                          ▼                 │
│                              ┌─────────────────────┐       │
│                              │ Edge WebView2       │       │
│                              │ Runtime             │       │
│                              └─────────────────────┘       │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 コンポーネント詳細

#### HTTP API Server
- Kestrel (ASP.NET Core) または最小HTTPサーバー
- REST API + WebSocket対応
- 認証トークンによるアクセス制御

#### Session Manager
- 複数WebView2インスタンスの管理
- Cookie/認証状態の永続化
- セッションタイムアウト処理

#### WebView2 Pool
- 複数インスタンスの並列実行
- インスタンスの再利用
- メモリ使用量の監視と制限

---

## 3. API設計

### 3.1 REST API

#### セッション管理

```http
POST /api/sessions
Content-Type: application/json
Authorization: Bearer {token}

{
  "profile": "default",      // プロファイル名（Cookie分離）
  "headless": true,          // ヘッドレスモード
  "userAgent": "...",        // オプション
  "viewport": {"width": 1920, "height": 1080}
}

Response:
{
  "sessionId": "sess_abc123",
  "status": "ready"
}
```

```http
DELETE /api/sessions/{sessionId}
```

#### ナビゲーション

```http
POST /api/sessions/{sessionId}/navigate
{
  "url": "https://example.com",
  "waitUntil": "networkidle",  // load | domcontentloaded | networkidle
  "timeout": 30000
}

Response:
{
  "status": "ok",
  "finalUrl": "https://example.com/redirected",
  "statusCode": 200,
  "loadTime": 1234
}
```

#### DOM操作

```http
POST /api/sessions/{sessionId}/evaluate
{
  "script": "document.querySelector('h1').textContent",
  "returnType": "string"  // string | json | void
}

Response:
{
  "result": "Page Title"
}
```

```http
POST /api/sessions/{sessionId}/query
{
  "selector": "div.product",
  "attributes": ["id", "data-price"],
  "textContent": true,
  "limit": 100
}

Response:
{
  "elements": [
    {"id": "prod-1", "data-price": "1000", "textContent": "Product 1"},
    ...
  ]
}
```

#### スナップショット

```http
POST /api/sessions/{sessionId}/snapshot
{
  "format": "aria",     // aria | html | text | markdown
  "selector": "main",   // オプション、特定要素のみ
  "maxDepth": 10
}

Response:
{
  "snapshot": "...",
  "url": "https://...",
  "title": "Page Title"
}
```

#### スクリーンショット

```http
POST /api/sessions/{sessionId}/screenshot
{
  "format": "png",           // png | jpeg
  "fullPage": false,
  "selector": "#main",       // オプション
  "quality": 80              // jpeg only
}

Response:
{
  "data": "base64...",
  "width": 1920,
  "height": 1080
}
```

#### 入力操作

```http
POST /api/sessions/{sessionId}/action
{
  "actions": [
    {"type": "click", "selector": "#submit"},
    {"type": "type", "selector": "#email", "text": "test@example.com"},
    {"type": "select", "selector": "#country", "value": "JP"},
    {"type": "wait", "ms": 1000},
    {"type": "waitForSelector", "selector": ".result"},
    {"type": "scroll", "y": 500},
    {"type": "hover", "selector": ".menu"}
  ]
}
```

#### Cookie管理

```http
GET /api/sessions/{sessionId}/cookies
?domain=example.com

POST /api/sessions/{sessionId}/cookies
{
  "cookies": [
    {"name": "session", "value": "abc", "domain": ".example.com"}
  ]
}

DELETE /api/sessions/{sessionId}/cookies
?name=session&domain=example.com
```

### 3.2 WebSocket API

リアルタイムイベント通知用:

```javascript
// 接続
ws://localhost:9400/ws?token={token}&sessionId={sessionId}

// サーバー→クライアント イベント
{
  "event": "navigation",
  "data": {"url": "...", "status": "loading"}
}

{
  "event": "console",
  "data": {"level": "log", "message": "..."}
}

{
  "event": "download",
  "data": {"filename": "...", "size": 1234, "path": "..."}
}

{
  "event": "dialog",
  "data": {"type": "alert", "message": "..."}
}
```

### 3.3 OpenClaw互換インターフェース

OpenClawの`browser`ツールと互換性のあるエンドポイント:

```http
POST /api/openclaw/snapshot
{
  "targetUrl": "https://...",
  "refs": "aria",
  "interactive": true
}

POST /api/openclaw/act
{
  "request": {
    "kind": "click",
    "ref": "button[Submit]"
  }
}
```

---

## 4. セキュリティ設計

### 4.1 認証・認可

```
┌─────────────────────────────────────────────┐
│  認証フロー                                  │
│                                             │
│  1. 初回起動時にランダムトークン生成         │
│  2. トークンをファイルに保存                 │
│     %LOCALAPPDATA%\WebViewBridge\token.txt  │
│  3. OpenClawがトークンを読み取り設定         │
│  4. 全リクエストでBearer認証必須             │
└─────────────────────────────────────────────┘
```

### 4.2 ネットワークセキュリティ

- **バインドアドレス**: `127.0.0.1` のみ（外部アクセス禁止）
- **HTTPS**: ローカルのみなのでHTTPで可（オプションでHTTPS対応）
- **CORS**: 無効（サーバー間通信のみ）

### 4.3 リソース制限

| リソース | 制限値 | 理由 |
|----------|--------|------|
| 同時セッション数 | 5 | メモリ保護 |
| セッションタイムアウト | 30分 | リソース解放 |
| リクエストサイズ | 10MB | DoS防止 |
| スクリプト実行時間 | 30秒 | ハング防止 |

### 4.4 サンドボックス

WebView2のセキュリティ機能を活用:
- プロセス分離
- ファイルシステムアクセス制限
- クリップボードアクセス制御

---

## 5. プロファイル管理

### 5.1 プロファイルの分離

```
%LOCALAPPDATA%\WebViewBridge\
├── profiles\
│   ├── default\           # デフォルトプロファイル
│   │   ├── Cookies
│   │   ├── Local Storage\
│   │   └── ...
│   ├── claude\            # Claude.ai専用
│   ├── google\            # Google系サービス
│   └── ecommerce\         # EC系サイト
├── token.txt
├── settings.json
└── logs\
```

### 5.2 プロファイル設定

```json
{
  "profiles": {
    "default": {
      "userAgent": null,
      "defaultTimeout": 30000,
      "blockAds": false
    },
    "stealth": {
      "userAgent": "Mozilla/5.0 ...",
      "blockAds": true,
      "blockTrackers": true
    }
  }
}
```

---

## 6. エラーハンドリング

### 6.1 エラーレスポンス形式

```json
{
  "error": {
    "code": "NAVIGATION_TIMEOUT",
    "message": "Navigation timed out after 30000ms",
    "details": {
      "url": "https://example.com",
      "timeout": 30000
    }
  }
}
```

### 6.2 エラーコード一覧

| コード | 説明 | 対処 |
|--------|------|------|
| `SESSION_NOT_FOUND` | セッションが存在しない | 新規セッション作成 |
| `SESSION_LIMIT_EXCEEDED` | セッション数上限 | 古いセッション削除 |
| `NAVIGATION_TIMEOUT` | ページ読み込みタイムアウト | タイムアウト延長/リトライ |
| `NAVIGATION_FAILED` | ナビゲーション失敗 | URL確認/ネットワーク確認 |
| `ELEMENT_NOT_FOUND` | 要素が見つからない | セレクタ確認/待機追加 |
| `SCRIPT_ERROR` | JavaScript実行エラー | スクリプト修正 |
| `UNAUTHORIZED` | 認証エラー | トークン確認 |
| `WEBVIEW_CRASHED` | WebView2クラッシュ | セッション再作成 |

### 6.3 自動リカバリー

```csharp
// WebView2クラッシュ時の自動復旧
webView.CoreWebView2.ProcessFailed += (s, e) => {
    if (e.ProcessFailedKind == CoreWebView2ProcessFailedKind.RenderProcessExited) {
        // 自動的に新しいWebView2を作成
        RecreateWebView();
        // 最後のURLに再ナビゲート
        Navigate(lastUrl);
    }
};
```

---

## 7. パフォーマンス最適化

### 7.1 WebView2プール

```
┌─────────────────────────────────────────────┐
│  WebView2 Pool                              │
│                                             │
│  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐  │
│  │ WV1 │ │ WV2 │ │ WV3 │ │ WV4 │ │ WV5 │  │
│  │BUSY │ │IDLE │ │BUSY │ │IDLE │ │ --- │  │
│  └─────┘ └─────┘ └─────┘ └─────┘ └─────┘  │
│                                             │
│  - アイドル状態のインスタンスを再利用        │
│  - 最大5インスタンスまで                    │
│  - 使用後はリセットしてプールに返却          │
└─────────────────────────────────────────────┘
```

### 7.2 リソース管理

- **メモリ監視**: 各WebView2のメモリ使用量を監視、閾値超過で強制終了
- **CPU監視**: 高CPU使用が続く場合は警告
- **ディスク**: キャッシュサイズ制限、定期クリーンアップ

### 7.3 キャッシュ戦略

```json
{
  "cache": {
    "enabled": true,
    "maxSize": "500MB",
    "ttl": 3600,
    "excludePatterns": [
      "**/api/**",
      "**/*.json"
    ]
  }
}
```

---

## 8. 技術スタック

### 8.1 採用技術

| 項目 | 技術 | 理由 |
|------|------|------|
| 言語 | C# (.NET 8) | WebView2との親和性、Claude Monitorで実績 |
| UI Framework | なし（コンソール/サービス） | ヘッドレス運用が主 |
| HTTP Server | ASP.NET Core Minimal API | 軽量、高性能 |
| WebSocket | SignalR または System.Net.WebSockets | リアルタイム通信 |
| WebView2 | Microsoft.Web.WebView2 | 本プロジェクトのコア |
| JSON | System.Text.Json | 高速、標準搭載 |
| Logging | Serilog | 構造化ログ |

### 8.2 プロジェクト構成

```
WebViewBridge/
├── src/
│   ├── WebViewBridge.Core/           # コアロジック
│   │   ├── Sessions/
│   │   │   ├── SessionManager.cs
│   │   │   ├── Session.cs
│   │   │   └── SessionOptions.cs
│   │   ├── WebView/
│   │   │   ├── WebViewPool.cs
│   │   │   ├── WebViewInstance.cs
│   │   │   └── WebViewActions.cs
│   │   └── Profiles/
│   │       ├── ProfileManager.cs
│   │       └── Profile.cs
│   │
│   ├── WebViewBridge.Api/            # HTTP API
│   │   ├── Controllers/
│   │   │   ├── SessionController.cs
│   │   │   ├── NavigationController.cs
│   │   │   ├── ActionController.cs
│   │   │   └── OpenClawController.cs
│   │   ├── Middleware/
│   │   │   ├── AuthMiddleware.cs
│   │   │   └── ErrorHandlingMiddleware.cs
│   │   └── Program.cs
│   │
│   └── WebViewBridge.App/            # エントリーポイント
│       ├── App.xaml
│       ├── App.xaml.cs
│       └── MainWindow.xaml           # 最小UI（トレイアイコン）
│
├── tests/
│   ├── WebViewBridge.Core.Tests/
│   └── WebViewBridge.Api.Tests/
│
├── docs/
│   ├── design.md
│   ├── use-cases.md
│   ├── api-reference.md
│   └── integration-guide.md
│
├── scripts/
│   └── install.ps1
│
└── README.md
```

---

## 9. デプロイメント

### 9.1 配布形式

1. **インストーラ (NSIS)**: Program Filesにインストール、サービス登録
2. **ポータブル版 (ZIP)**: 任意フォルダで実行
3. **Windowsサービス**: バックグラウンド常駐

### 9.2 起動モード

```bash
# コンソールモード（デバッグ用）
WebViewBridge.exe --console

# トレイアイコンモード（通常使用）
WebViewBridge.exe

# Windowsサービスモード
WebViewBridge.exe --service

# ポート指定
WebViewBridge.exe --port 9400
```

### 9.3 OpenClaw統合

```yaml
# OpenClaw config.yaml
browser:
  provider: webview-bridge
  endpoint: http://localhost:9400
  token: ${WEBVIEW_BRIDGE_TOKEN}
```

---

## 10. ロードマップ

### Phase 1: MVP (1週間)
- [ ] 基本HTTP API
- [ ] 単一WebView2セッション
- [ ] navigate, evaluate, screenshot
- [ ] Bearer認証

### Phase 2: 機能拡充 (2週間)
- [ ] 複数セッション対応
- [ ] WebView2プール
- [ ] プロファイル管理
- [ ] Cookie管理API
- [ ] WebSocket対応

### Phase 3: OpenClaw統合 (1週間)
- [ ] OpenClaw互換API
- [ ] ARIAスナップショット
- [ ] アクション実行

### Phase 4: 安定化 (1週間)
- [ ] エラーハンドリング強化
- [ ] 自動リカバリー
- [ ] パフォーマンスチューニング
- [ ] インストーラ作成

---

## 11. 代替案との比較

| 項目 | WebView Bridge | Puppeteer | Playwright | Browser Relay |
|------|---------------|-----------|------------|---------------|
| 安定性 | ◎ | △ (WSL2で不安定) | ○ | ○ |
| Cloudflare | ◎ (バイパス) | △ | △ | ◎ |
| セットアップ | ○ | ◎ | ◎ | △ (手動操作) |
| ヘッドレス | ◎ | ◎ | ◎ | × |
| リソース消費 | ○ | △ | △ | ◎ |
| Windows依存 | × (必須) | ◎ | ◎ | ◎ |
| 並列実行 | ○ | ○ | ◎ | △ |

**結論**: Windows環境でCloudflareバイパスが必要な安定したスクレイピングには最適解。

---

## 12. 参考資料

- [WebView2 Documentation](https://docs.microsoft.com/en-us/microsoft-edge/webview2/)
- [OpenClaw Browser Tool](https://docs.openclaw.ai/tools/browser)
- [Chrome DevTools Protocol](https://chromedevtools.github.io/devtools-protocol/)
