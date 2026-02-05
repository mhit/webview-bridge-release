# WebView Bridge Protocol v2 (WBP2) 設計書

> 最終更新: 2026-02-05
>
> ステータス: 設計中

---

## 1. 概要

### 1.1 目的

WebView Bridge Protocol v2 (WBP2) は、**AIエージェントおよびプログラムからのブラウザ自動化に最適化された**新しいプロトコルです。

既存のPlaywright/Puppeteer/Seleniumはヒューマンオペレーターがスクリプトを書くことを前提に設計されていますが、WBP2はAIが直接操作することを前提とします。

### 1.2 設計原則

| 原則 | 説明 |
|------|------|
| **宣言的ゴール** | 「クリックして」ではなく「このデータを取得して」 |
| **イベント駆動** | ポーリングではなく、状態変化をプッシュ通知 |
| **セッション永続化** | 認証状態を維持し、再ログインを最小化 |
| **スマート待機** | タイムアウトではなく、条件が満たされたら即座に応答 |
| **エラー自己回復** | 一時的なエラーは内部でリトライ |

### 1.3 現状の問題点

```
┌──────────────────────────────────────────────────────────────────┐
│                   現在のフレームワークの問題                      │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│ [1. ポーリングの無駄]                                            │
│     await page.waitForSelector('.element', { timeout: 30000 });  │
│     // 内部で50msごとにDOMをチェック → CPU/時間の浪費            │
│                                                                  │
│ [2. セッション揮発性]                                            │
│     // サーバー再起動でセッション消失                            │
│     // 認証状態が失われ、再ログインが必要                        │
│                                                                  │
│ [3. プロファイル乱立]                                            │
│     // 「x_auth」「yahoo_news」「rakuten_search」...             │
│     // 管理が煩雑、重複ログインが発生                            │
│                                                                  │
│ [4. 低レベル操作の要求]                                          │
│     // AIは「ログインして」を理解するが、                        │
│     // 「#username入力 → #password入力 → #submit クリック」     │
│     // という手順を毎回指定する必要がある                        │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

---

## 2. 現状分析

### 2.1 現在サポートしているプロトコル

| プロトコル | 実装状態 | 対象ユーザー | 課題 |
|-----------|---------|-------------|------|
| **REST API (native)** | ✅ 完了 | プログラマー | セッション管理が手動 |
| **WebDriver (W3C)** | ✅ 完了 | Selenium | 低レベル操作のみ |
| **MCP** | ✅ 完了 | AI (Claude等) | セッション概念が曖昧 |
| **CDP** | ⚠️ 部分実装 | Puppeteer | WebSocket未対応 |

### 2.2 現在のセッション管理

```rust
// 現在の実装
pub struct SessionManager {
    sessions: HashMap<String, SessionHandle>,  // メモリ上のみ
    idle_timeout: Duration,
}

// 問題点:
// 1. サーバー再起動でsessionsはクリア
// 2. プロファイルは永続化されるが、セッションIDは毎回新規
// 3. 同じプロファイルでも複数セッションが作成可能（意図しない重複）
```

### 2.3 現在のスクリーンショット機能

| 機能 | 実装状態 |
|------|---------|
| ビューポートキャプチャ | ✅ (テキスト描画方式) |
| フルページキャプチャ | ❌ |
| 要素指定キャプチャ | ❌ |
| デバイスエミュレーション | ❌ |
| 画像待機 | ❌ |

---

## 3. WBP2 プロトコル設計

### 3.1 セッション管理 v2

```
┌─────────────────────────────────────────────────────────────────┐
│                    セッション管理モデル v2                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  [Named Sessions]                                               │
│  ├── "x" ─────────────────→ プロファイル: x_authenticated       │
│  │                          認証状態: ログイン済み              │
│  │                          最終使用: 2026-02-05 15:30          │
│  │                          自動延命: true                      │
│  │                                                              │
│  ├── "google" ────────────→ プロファイル: google_workspace      │
│  │                          認証状態: ログイン済み              │
│  │                                                              │
│  └── "default" ───────────→ プロファイル: default               │
│                             認証状態: 匿名                      │
│                                                                 │
│  [Session Lifecycle]                                            │
│  ├── acquire(name) ─→ 既存があれば再利用、なければ作成         │
│  ├── release(name) ─→ セッションを解放（プロファイルは保持）   │
│  ├── destroy(name) ─→ セッション＋プロファイルを削除           │
│  └── list() ────────→ 全セッションの状態一覧                   │
│                                                                 │
│  [Persistence]                                                  │
│  ├── sessions.json ─→ セッション状態を永続化                   │
│  ├── 起動時に復元 ──→ 名前付きセッションを再構築               │
│  └── warm_up ───────→ よく使うセッションを事前起動             │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

#### API設計

```http
# セッション取得（再利用 or 作成）
POST /v2/session/acquire
{
    "name": "x",                         # 名前付きセッション
    "profile": "x_authenticated",        # プロファイル名（オプション）
    "reuse": true,                       # 既存セッション再利用
    "create_if_missing": true,           # なければ作成
    "headless": false,                   # 表示モード
    "auth_check": {                      # 認証状態チェック（オプション）
        "url": "https://x.com/home",
        "logged_in_selector": "[data-testid='SideNav']",
        "login_required_selector": "[href='/login']"
    }
}

# レスポンス
{
    "session_id": "x",                   # 名前がそのままID
    "is_new": false,                     # 既存を再利用
    "profile": "x_authenticated",
    "auth_status": {
        "logged_in": true,
        "checked_at": "2026-02-05T15:30:00Z"
    }
}
```

### 3.2 イベント駆動通知

```
┌─────────────────────────────────────────────────────────────────┐
│                    イベント駆動アーキテクチャ                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  [MutationObserver インジェクション]                            │
│                                                                 │
│  クライアント                    サーバー           WebView2    │
│      │                             │                   │        │
│      │ POST /v2/wait               │                   │        │
│      │ {selector: ".tweet"}        │                   │        │
│      │ ─────────────────────────→  │                   │        │
│      │                             │ MutationObserver  │        │
│      │                             │ ─────────────────→│        │
│      │                             │                   │        │
│      │                             │    (DOM変化)      │        │
│      │                             │ ←────────────────│        │
│      │                             │                   │        │
│      │    {found: true, ...}       │                   │        │
│      │ ←─────────────────────────  │                   │        │
│      │                             │                   │        │
│                                                                 │
│  [ポーリング vs イベント駆動]                                   │
│                                                                 │
│  ポーリング (現在):                                             │
│    10秒待機 = 200回のDOM検索 (50ms間隔)                         │
│                                                                 │
│  イベント駆動 (WBP2):                                           │
│    10秒待機 = 1回のMutationObserver登録 + 1回のコールバック    │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

#### API設計

```http
# スマート待機
POST /v2/wait
{
    "session": "x",
    "selector": ".tweet",
    "condition": "present",    # present | visible | stable | text_contains
    "timeout": 10000,
    "extract": ["text", "href"]  # 見つかったら同時に抽出
}

# 条件が満たされた時点で即座にレスポンス
{
    "found": true,
    "elements": [
        {"text": "...", "href": "..."}
    ],
    "elapsed_ms": 234
}
```

### 3.3 スマートスクリーンショット

```http
POST /v2/screenshot
{
    "session": "x",
    "type": "element",              # viewport | fullpage | element
    "selector": ".main-content",    # type=element時に指定
    "device": {                     # デバイスエミュレーション
        "preset": "iPhone 14 Pro",  # プリセット使用
        # または
        "width": 393,
        "height": 852,
        "scale_factor": 3,
        "user_agent": "...",
        "touch": true
    },
    "wait_for_images": true,        # 画像読み込み完了まで待機
    "hide_selectors": [".ads", ".popup"],  # 非表示にする要素
    "format": "png",                # png | jpeg | webp
    "quality": 80                   # jpeg/webp時のクオリティ
}
```

#### フルページスクリーンショット実装

```
┌─────────────────────────────────────────────────────────────────┐
│                 フルページスクリーンショット                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  [実装オプション]                                               │
│                                                                 │
│  A. スクロール＋スティッチング                                  │
│     1. ページ総高さを取得: document.scrollingElement.scrollHeight│
│     2. ビューポート高さ分スクロール                             │
│     3. 各ビューポートをキャプチャ                               │
│     4. 画像を縦に結合                                           │
│     Pros: WebView2ネイティブ機能で実現可能                      │
│     Cons: スクロールで動的変化する要素は不正確                  │
│                                                                 │
│  B. CDP Page.captureScreenshot                                  │
│     WebView2のCallDevToolsProtocolMethodAsyncを使用            │
│     { "captureBeyondViewport": true }                           │
│     Pros: 一度のキャプチャで完了                                │
│     Cons: WebView2でのCDP対応が必要                             │
│                                                                 │
│  C. html2canvas (外部ライブラリ)                                │
│     ページにhtml2canvasを注入してレンダリング                   │
│     Pros: CSSスタイルを正確に再現                               │
│     Cons: 外部依存、大きなページでは遅い                        │
│                                                                 │
│  推奨: A (スクロール方式) をデフォルト、B (CDP) をオプション    │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 3.4 宣言的ゴールAPI

```http
# 高レベル操作
POST /v2/goal
{
    "session": "x",
    "goal": "extract_data",
    "target": {
        "description": "Get the latest 10 tweets about WebView2",
        "selector": "article[data-testid='tweet']",
        "fields": ["author", "text", "timestamp", "likes"],
        "limit": 10
    },
    "strategy": {
        "wait_for": "networkidle",
        "scroll_to_load": true,
        "retry_on_empty": 3
    }
}

# AIが解釈しやすいレスポンス
{
    "success": true,
    "data": [
        {
            "author": "@example",
            "text": "WebView2 is amazing...",
            "timestamp": "2h",
            "likes": 42
        },
        ...
    ],
    "metadata": {
        "total_found": 10,
        "scroll_count": 2,
        "elapsed_ms": 3456
    }
}
```

---

## 4. 実装ロードマップ

### Phase 7: セッション管理 v2 (優先度: 最高)

| タスク | 詳細 | 工数 |
|--------|------|------|
| 7.1 名前付きセッション | `acquire`/`release`/`destroy` API | 4h |
| 7.2 セッション永続化 | sessions.json + 起動時復元 | 4h |
| 7.3 認証状態チェック | ログイン/ログアウト検出 | 2h |
| 7.4 セッションプール | warm_up、自動クリーンアップ | 3h |

### Phase 8: イベント駆動待機 (優先度: 高)

| タスク | 詳細 | 工数 |
|--------|------|------|
| 8.1 MutationObserver注入 | DOM変化監視スクリプト | 3h |
| 8.2 スマート待機API | `/v2/wait` 実装 | 4h |
| 8.3 WebSocket通知 | リアルタイムイベント配信 | 6h |

### Phase 9: スクリーンショット v2 (優先度: 高)

| タスク | 詳細 | 工数 |
|--------|------|------|
| 9.1 要素指定キャプチャ | セレクター指定で部分撮影 | 3h |
| 9.2 フルページキャプチャ | スクロール+スティッチング | 6h |
| 9.3 デバイスエミュレーション | ビューポート/UA変更 | 4h |
| 9.4 画像待機 | img.complete チェック | 2h |

### Phase 10: 宣言的ゴール (優先度: 中)

| タスク | 詳細 | 工数 |
|--------|------|------|
| 10.1 ゴールパーサー | 高レベル指示の解釈 | 4h |
| 10.2 自動リトライ | エラー時の自己回復 | 3h |
| 10.3 プリセットフロー | 「ログインして」等の定義 | 4h |

---

## 5. 既存プロトコルの今後

### 5.1 対応方針

| プロトコル | 方針 | 理由 |
|-----------|------|------|
| **REST API v1** | 維持 (deprecated) | 後方互換性 |
| **REST API v2** | 新規開発 | AIファースト設計 |
| **WebDriver** | 維持 | Selenium互換、テスト用途 |
| **MCP** | 拡張 | AI連携の主要インターフェース |
| **CDP** | 部分維持 | 高度なデバッグ用途 |

### 5.2 MCP/ACP統合

```
┌─────────────────────────────────────────────────────────────────┐
│                    MCP/ACP統合アーキテクチャ                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  [MCP Tools - 拡張版]                                           │
│                                                                 │
│  ├── browse(url, session?)                                      │
│  │   → 名前付きセッションでナビゲート                          │
│  │                                                              │
│  ├── authenticate(session, site)                                │
│  │   → 認証状態チェック、必要なら再ログイン通知                │
│  │                                                              │
│  ├── extract(session, selector, fields)                         │
│  │   → データ抽出（待機込み）                                  │
│  │                                                              │
│  ├── screenshot(session, options)                               │
│  │   → フルページ/要素/デバイス対応                            │
│  │                                                              │
│  └── act(session, action, target)                               │
│      → クリック/入力/スクロール                                │
│                                                                 │
│  [ACP Resources]                                                │
│                                                                 │
│  ├── session://x/status                                         │
│  │   → セッション状態（認証含む）                              │
│  │                                                              │
│  ├── session://x/page                                           │
│  │   → 現在のページ内容                                        │
│  │                                                              │
│  └── session://list                                             │
│      → 全セッション一覧                                        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 6. 悲観的リスク評価

### 6.1 技術リスク

| リスク | 影響 | 緩和策 |
|--------|------|--------|
| WebView2のCDP対応が不完全 | フルページSSが困難 | スクロール方式をフォールバック |
| セッション永続化でのCookie期限切れ | 再ログイン発生 | 認証状態チェックで事前検出 |
| 大量セッションでのメモリ消費 | システム不安定 | プールサイズ制限、LRU削除 |
| WebSocketのコネクション管理 | リソースリーク | タイムアウト + 自動切断 |

### 6.2 互換性リスク

| リスク | 影響 | 緩和策 |
|--------|------|--------|
| v1 API破壊 | 既存利用者への影響 | v1は維持、v2は別パス |
| MCP仕様変更 | Claude連携不可 | 抽象レイヤーで吸収 |
| サイト側のbot対策 | スクレイピング失敗 | プロファイル分離、UA偽装 |

---

## 7. 次のアクション

### 即座に実行 (このセッション)

1. ✅ 設計ドキュメント作成 (本ドキュメント)
2. ⬜ Plans.md更新 (Phase 7-10追加)
3. ⬜ 名前付きセッション実装 (Phase 7.1)

### 短期 (今週中)

4. セッション永続化実装
5. 認証状態チェックAPI
6. MCP Tools拡張

### 中期 (今月中)

7. スクリーンショットv2
8. イベント駆動待機
9. デバイスエミュレーション

---

## 付録: デバイスプリセット

```json
{
  "devices": {
    "iPhone 14 Pro": {
      "width": 393,
      "height": 852,
      "deviceScaleFactor": 3,
      "mobile": true,
      "userAgent": "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15"
    },
    "iPhone 14 Pro Max": {
      "width": 430,
      "height": 932,
      "deviceScaleFactor": 3,
      "mobile": true,
      "userAgent": "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15"
    },
    "iPad Pro": {
      "width": 1024,
      "height": 1366,
      "deviceScaleFactor": 2,
      "mobile": true,
      "userAgent": "Mozilla/5.0 (iPad; CPU OS 16_0 like Mac OS X) AppleWebKit/605.1.15"
    },
    "Galaxy S23": {
      "width": 360,
      "height": 780,
      "deviceScaleFactor": 3,
      "mobile": true,
      "userAgent": "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 Chrome/116.0.0.0 Mobile"
    },
    "Desktop HD": {
      "width": 1920,
      "height": 1080,
      "deviceScaleFactor": 1,
      "mobile": false,
      "userAgent": null
    }
  }
}
```
