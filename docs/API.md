# REST API リファレンス

WebView Bridge v3.5 REST API。デフォルト: `http://localhost:9400`

## 共通

- Content-Type: `application/json`
- レスポンス: JSON形式

---

## 認証

WebView Bridge は起動時にBearerトークンを自動生成し、コンソールに出力します。

### トークン認証
- REST API: `Authorization: Bearer <token>` ヘッダーが必要
- ダッシュボード: HTMLに `window.__WB_TOKEN` として自動注入

### 公開エンドポイント（認証不要）
| エンドポイント | 説明 |
|---------------|------|
| `GET /` | ダッシュボード HTML |
| `GET /health` | ヘルスチェック |
| `GET /favicon.ico` | ファビコン |
| `GET /assets/icon.png` | アプリアイコン |

その他すべてのエンドポイントは Bearer トークン認証が必要です。

---

## システム

| メソッド | パス | 説明 |
|---------|------|------|
| GET | `/` | ダッシュボード (HTML) |
| GET | `/health` | ヘルスチェック |

### GET /
ダッシュボード UI を返します。

- 11ページのSPAインターフェース（CSS/HTML/JS単一ファイル）
- リアルタイム監視（10秒間隔の自動更新）
- セッション/スクリーンショット/ダウンロード/AI/MCP管理
- APIテスター、バッチ実行、オートメーション

認証: 不要（HTMLロード後、APIコールにはトークンが自動付与）

---

## セッション管理

| メソッド | パス | 説明 |
|---------|------|------|
| POST | `/session/acquire` | セッション作成 |
| POST | `/session/release` | セッション解放 |
| DELETE | `/session/destroy` | セッション完全削除 |
| POST | `/session/cleanup` | 期限切れセッションのクリーンアップ |
| POST | `/session/clone` | セッション複製 |
| POST | `/session/visibility` | ウィンドウ表示/非表示切替 |
| POST | `/session/focus` | ウィンドウフォーカス |
| GET | `/session/list` | セッション一覧 |
| GET | `/session/stats` | セッション統計 |
| GET | `/session/:name` | セッション詳細取得 |
| POST | `/session/state/url` | 現在のURL取得/設定 |
| GET | `/session/state/history` | URL履歴取得 |
| POST | `/session/import` | Cookie インポート |
| GET | `/session/import/profiles` | インポート可能なプロファイル一覧 |

### POST /session/acquire

```json
// リクエスト
{"name": "my-session", "headless": false, "restore": true, "ttl_hours": 168}

// レスポンス
{"session": "my-session", "id": "uuid", "status": "active"}
```

### POST /session/release

```json
{"name": "my-session"}
```

---

## ナビゲーション

| メソッド | パス | 説明 |
|---------|------|------|
| POST | `/navigate` | ページ遷移 |

```json
// リクエスト
{"session": "my-session", "url": "https://example.com", "wait_for": "stable"}

// レスポンス
{"success": true, "url": "https://example.com", "title": "Example"}
```

---

## ブラウザ操作

| メソッド | パス | 説明 |
|---------|------|------|
| POST | `/click` | 要素クリック |
| POST | `/type` | テキスト入力 |
| POST | `/execute` | JavaScript実行 |
| POST | `/wait` | 条件待機 |

### POST /click

```json
{"session": "my-session", "selector": "#submit"}
```

### POST /type

```json
{"session": "my-session", "selector": "#search", "text": "query", "clear": true}
```

### POST /execute

```json
{"session": "my-session", "script": "return document.title"}
```

---

## スクリーンショット

| メソッド | パス | 説明 |
|---------|------|------|
| POST | `/screenshot` | スクリーンショット取得 |
| GET | `/screenshot/devices` | 利用可能デバイス一覧 |

```json
// リクエスト
{"session": "my-session", "full_page": true}
```

---

## Goal/マクロ

| メソッド | パス | 説明 |
|---------|------|------|
| POST | `/goal` | ゴールベース操作実行 |
| GET | `/goal/flows` | フロー一覧 |
| POST | `/macro` | マクロ実行 |
| GET | `/macro/list` | マクロ一覧 |
| POST | `/macro/register` | マクロ登録 |
| POST | `/macro/detect-spa` | SPA検出 |

---

## メディア

| メソッド | パス | 説明 |
|---------|------|------|
| POST | `/media/images` | 画像収集 |
| POST | `/media/youtube/subtitles` | YouTube字幕取得 |
| POST | `/media/youtube/download` | YouTubeダウンロード |
| POST | `/media/analyze` | メディア分析 |
| GET | `/media/files/:ref` | ファイル一覧 |
| GET | `/media/screenshots` | スクリーンショット一覧 |
| GET | `/media/screenshots/:session/:filename` | スクリーンショット取得 |
| POST | `/media/persist` | メディア永続化 |
| POST | `/media/extend` | TTL延長 |

---

## AI

| メソッド | パス | 説明 |
|---------|------|------|
| POST | `/ai/config` | AI設定更新 |
| GET | `/ai/config` | AI設定取得 |
| POST | `/ai/login` | AIログイン |
| POST | `/ai/images/analyze` | 画像分析(Vision) |
| POST | `/ai/extract` | AI抽出 |
| GET | `/ai/usage` | AI使用量統計 |

---

## ダウンロード

| メソッド | パス | 説明 |
|---------|------|------|
| POST | `/download/trigger` | ダウンロード開始 |
| GET | `/download/status/:id` | ダウンロード状態 |
| POST | `/download/batch` | バッチダウンロード |

---

## ストレージ

| メソッド | パス | 説明 |
|---------|------|------|
| GET | `/storage/status` | ストレージ状態 |
| POST | `/storage/cleanup` | ストレージクリーンアップ |
| POST | `/config/storage` | ストレージ設定 |

---

## 設定

| メソッド | パス | 説明 |
|---------|------|------|
| GET | `/v2/config` | 設定取得 |
| POST | `/v2/config` | 設定更新 |
| POST | `/v2/ai/test` | AI接続テスト |

---

## ジョブ

| メソッド | パス | 説明 |
|---------|------|------|
| GET | `/jobs/:id` | ジョブ状態取得 |
| DELETE | `/jobs/:id` | ジョブキャンセル |
| GET | `/jobs` | ジョブ一覧 |

---

## バッチ

| メソッド | パス | 説明 |
|---------|------|------|
| POST | `/batch` | 複数API一括実行 |

---

## MCP

| メソッド | パス | 説明 |
|---------|------|------|
| POST | `/mcp` | MCPツール呼び出し |
| GET | `/mcp/tools` | ツール一覧 |

```json
// リクエスト
{"tool": "navigate", "session": "demo", "url": "https://example.com"}

// レスポンス
{"success": true, "content": [{"type": "text", "text": "..."}]}
```

---

## WebSocket

| パス | 説明 |
|------|------|
| `/ws` | リアルタイムイベント |

イベント: ページ遷移、DOM変更、ネットワーク通信等をリアルタイム配信。

---

## エラーコード

| コード | 名前 | HTTP | 説明 |
|--------|------|------|------|
| WBP2_001 | SESSION_NOT_FOUND | 404 | セッションが見つからない |
| WBP2_002 | SESSION_BUSY | 409 | セッションが使用中 |
| WBP2_003 | SESSION_CLOSED | 410 | セッションが閉じている |
| WBP2_090 | INVALID_REQUEST | 400 | リクエスト不正 |
| WBP2_091 | MISSING_PARAMETER | 400 | 必須パラメータ不足 |
| WBP2_099 | INTERNAL_ERROR | 500 | 内部エラー |
