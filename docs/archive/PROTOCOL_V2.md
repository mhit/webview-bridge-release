# WebView Bridge Protocol v2 (WBP2) 設計書

> 最終更新: 2026-02-05 (査読修正版)
>
> ステータス: 設計中

---

## 1. 概要

### 1.1 目的

WebView Bridge Protocol v2 (WBP2) は、**AIエージェントおよびプログラムからのブラウザ自動化に最適化された**新しいプロトコルです。

既存のPlaywright/Puppeteer/Seleniumはヒューマンオペレーターがスクリプトを書くことを前提に設計されていますが、WBP2はAIが直接操作することを前提とします。

### 1.2 設計原則

#### コアプロトコル原則

| 原則 | 説明 |
|------|------|
| **宣言的ゴール** | 「クリックして」ではなく「このデータを取得して」 |
| **イベント駆動** | ポーリングではなく、状態変化をプッシュ通知 |
| **セッション永続化** | 認証状態を維持し、再ログインを最小化 |
| **スマート待機** | タイムアウトではなく、条件が満たされたら即座に応答 |
| **エラー自己回復** | 一時的なエラーは内部でリトライ |

#### 効率化原則 (→ 詳細: セクション9)

| 原則 | 説明 |
|------|------|
| **通信回数最小化** | マクロで複数操作を1リクエストに集約 |
| **ファイル参照パターン** | 大きなファイルは参照URLで返却 |
| **AIコンテキスト効率** | 1ターンで完結する設計 |

#### AI統合原則 (→ 詳細: セクション14)

| 原則 | 説明 |
|------|------|
| **段階的AI利用** | セレクター → パターン → AI の順に試行 |
| **コスト最適化** | 軽量モデルを優先、必要時のみ高機能モデル |
| **プライバシー保護** | センシティブ情報は自動マスク |

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

### 1.4 用語定義

| カテゴリ | 用語 | 説明 |
|---------|------|------|
| **ブラウザ** | `session` | ブラウザセッション（名前付き、例: "rakuten"） |
| | `profile` | ブラウザプロファイル（Cookie等永続化） |
| | `webview` | WebView2インスタンス |
| **ファイル** | `file_ref` | ファイル参照ID（例: "downloads_001"） |
| | `files_url` | ファイル一覧URL |
| **ジョブ** | `job_id` | 非同期ジョブのID |
| | `status` | ジョブ状態 (pending\|running\|completed\|failed) |
| | `progress` | 進捗情報（percent、ETA等） |
| **AI** | `ai_mode` | AI機能使用フラグ |
| | `ai_config` | AI設定（APIキー、モデル等） |

> 詳細な用語定義は [15.2 用語統一](#152-用語統一) を参照

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
    "session": "x",                      # セッション名（統一キー）
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

### Phase 11: マクロスクリプト (優先度: 高)

| タスク | 詳細 | 工数 |
|--------|------|------|
| 11.1 JSマクロエンジン | WebView2内でJS実行、複数ステップ一括 | 6h |
| 11.2 よくあるパターン定義 | リスト抽出、ページネーション等 | 4h |
| 11.3 SPA対応 | クライアントサイドレンダリング待機 | 3h |
| 11.4 マクロ登録API | カスタムマクロの保存・実行 | 3h |

### Phase 12: メディア収集 (優先度: 中)

| タスク | 詳細 | 工数 |
|--------|------|------|
| 12.1 画像一括収集 | ページ内画像の抽出・ダウンロード | 4h |
| 12.2 YouTube字幕取得 | yt-dlp連携、字幕ファイル生成 | 4h |
| 12.3 動画ダウンロード | yt-dlp経由の動画取得 | 4h |
| 12.4 メディアキャッシュ | 重複ダウンロード防止 | 2h |

### Phase 13: レガシー廃止 (優先度: 低)

| タスク | 詳細 | 工数 |
|--------|------|------|
| 13.1 v1 API非推奨化 | deprecated警告追加 | 2h |
| 13.2 移行ガイド | v1→v2移行ドキュメント | 4h |
| 13.3 v1停止 | 移行期間後のv1削除 | 2h |

### Phase 14: AI統合 (Gemini) (優先度: 中)

| タスク | 詳細 | 工数 |
|--------|------|------|
| 14.1 AI設定API | `/v2/config/ai` 実装 | 3h |
| 14.2 AI画像評価 | スクリーンショット分析 | 6h |
| 14.3 自動ログイン | AI-in-the-Loop認証 | 8h |
| 14.4 動的要素検出 | AIによるセレクター推論 | 6h |

### Phase 15: ダウンロード & ストレージ (優先度: 中)

| タスク | 詳細 | 工数 |
|--------|------|------|
| 15.1 ブラウザダウンロード | WebView2ダウンロードイベント | 6h |
| 15.2 ファイル参照システム | file_ref管理、TTL | 4h |
| 15.3 ストレージ管理 | 容量監視、自動削除 | 4h |
| 15.4 バッチダウンロード | 複数ファイル一括 | 4h |

### Phase 16: 通信設計 (Webhook/Batch) (優先度: 中)

| タスク | 詳細 | 工数 |
|--------|------|------|
| 16.1 非同期ジョブ統一 | `/v2/jobs/:id` 実装 | 4h |
| 16.2 Webhook通知 | 長時間処理完了通知 | 6h |
| 16.3 バッチリクエスト | `/v2/batch` 実装 | 4h |
| 16.4 WebSocketイベント | リアルタイム通知 | 4h |

---

## 5. マクロスクリプト設計

### 5.1 設計思想

```
┌─────────────────────────────────────────────────────────────────┐
│                    マクロスクリプト設計思想                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  [問題: 通信オーバーヘッド]                                     │
│                                                                 │
│  従来のアプローチ:                                              │
│    AI → navigate(url)     → サーバー → WebView2                │
│    AI → waitForSelector() → サーバー → WebView2                │
│    AI → extract(selector) → サーバー → WebView2                │
│    AI → click(next)       → サーバー → WebView2                │
│    AI → waitForSelector() → サーバー → WebView2                │
│    AI → extract(selector) → サーバー → WebView2                │
│    ...                                                          │
│    → 6回の通信、AIコンテキスト消費大                            │
│                                                                 │
│  マクロアプローチ:                                              │
│    AI → execute_macro({                                         │
│           type: "paginated_list",                               │
│           selector: ".item",                                    │
│           next_button: ".next",                                 │
│           max_pages: 5                                          │
│         })                                                      │
│    → 1回の通信、WebView2内で全処理完結                          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 5.2 JSマクロエンジン

WebView2内で動作するJavaScriptエンジン。複数のステップを一括実行。

```javascript
// WebView2内で実行されるマクロ例
(async function(config) {
    const results = [];
    let pageNum = 1;
    
    while (pageNum <= config.max_pages) {
        // 要素が表示されるまで待機
        await waitFor(config.selector, { timeout: 10000 });
        
        // データ抽出
        const items = document.querySelectorAll(config.selector);
        items.forEach(item => {
            results.push({
                text: item.innerText,
                href: item.querySelector('a')?.href,
                img: item.querySelector('img')?.src
            });
        });
        
        // 次のページへ
        const nextBtn = document.querySelector(config.next_button);
        if (!nextBtn || nextBtn.disabled) break;
        
        nextBtn.click();
        await waitForNavigation();
        pageNum++;
    }
    
    return results;
})(__CONFIG__);
```

### 5.3 よくあるパターン (プリセットマクロ)

#### リスト抽出 (extract_list)

```http
POST /v2/macro
{
    "session": "default",
    "macro": "extract_list",
    "config": {
        "item_selector": ".product-card",
        "fields": {
            "name": ".product-name",
            "price": ".product-price",
            "image": "img@src",        // @attributeで属性取得
            "link": "a@href"
        },
        "filter": {                     // オプション: 不要な要素を除外
            "exclude_text": ["広告", "PR"],
            "min_price": 1000
        },
        "transform": {                  // オプション: データ変換
            "price": "parseInt(value.replace(/[^0-9]/g, ''))"
        }
    }
}

# レスポンス
{
    "success": true,
    "items": [
        {"name": "商品A", "price": 1980, "image": "...", "link": "..."},
        {"name": "商品B", "price": 2480, "image": "...", "link": "..."},
        ...
    ],
    "count": 24,
    "filtered_count": 2  // 除外された数
}
```

#### ページネーション (paginated_extract)

```http
POST /v2/macro
{
    "session": "default",
    "macro": "paginated_extract",
    "config": {
        "item_selector": ".search-result",
        "fields": {...},
        "pagination": {
            "type": "click",           // click | scroll | url_param
            "next_button": ".next-page",
            "max_pages": 10,
            "wait_after_click": 2000   // ms
        },
        // または無限スクロール対応
        "pagination": {
            "type": "scroll",
            "scroll_container": "window",
            "max_scrolls": 20,
            "no_new_items_threshold": 3  // 新しいアイテムが3回なければ終了
        }
    }
}
```

#### SPA待機 (wait_for_spa)

```http
POST /v2/macro
{
    "session": "default",
    "macro": "wait_for_spa",
    "config": {
        "trigger": {
            "action": "click",
            "selector": "#load-more"
        },
        "wait_for": {
            "type": "mutation",        // mutation | network_idle | element
            "selector": ".new-content",
            "timeout": 10000
        },
        "extract_after": {
            "selector": ".item",
            "fields": {...}
        }
    }
}
```

### 5.4 カスタムマクロ登録

```http
# マクロ登録
POST /v2/macro/register
{
    "name": "rakuten_product_search",
    "description": "楽天市場の商品検索結果を取得",
    "script": "
        (async function(config) {
            // 検索
            const input = document.querySelector('input[name=\"sitem\"]');
            input.value = config.keyword;
            input.form.submit();
            
            await waitFor('.searchresultitem', { timeout: 15000 });
            
            // 抽出
            const items = [];
            document.querySelectorAll('.searchresultitem').forEach(el => {
                items.push({
                    name: el.querySelector('.title')?.innerText,
                    price: el.querySelector('.price')?.innerText,
                    shop: el.querySelector('.shopname')?.innerText,
                    url: el.querySelector('a')?.href
                });
            });
            
            return items;
        })(__CONFIG__);
    ",
    "config_schema": {
        "keyword": { "type": "string", "required": true }
    }
}

# 登録したマクロの実行
POST /v2/macro
{
    "session": "rakuten",
    "macro": "rakuten_product_search",
    "config": {
        "keyword": "ゲーミングマウス"
    }
}
```

---

## 6. メディア収集設計

### 6.1 画像一括収集

```http
POST /v2/media/images
{
    "session": "default",
    "source": {
        "type": "page",              // page | selector | urls
        "selector": ".gallery img",  // type=selectorの場合
        "urls": ["..."]              // type=urlsの場合
    },
    "filter": {
        "min_width": 200,
        "min_height": 200,
        "exclude_patterns": ["icon", "logo", "avatar"]
    },
    "download": {
        "format": "original",        // original | jpeg | png | webp
        "max_size": 5000000,         // 5MB
        "concurrent": 5
    },
    "output": {
        "type": "base64",            // base64 | file | zip
        "path": "/downloads/images"  // type=file/zipの場合
    }
}

# レスポンス
{
    "success": true,
    "images": [
        {
            "url": "https://example.com/image1.jpg",
            "width": 800,
            "height": 600,
            "size": 125000,
            "data": "iVBORw0KGgo..."  // type=base64の場合
        },
        ...
    ],
    "total": 15,
    "downloaded": 12,
    "filtered": 3
}
```

### 6.2 YouTube字幕取得

```http
POST /v2/media/youtube/subtitles
{
    "url": "https://www.youtube.com/watch?v=xxxxx",
    "languages": ["ja", "en"],        // 優先順位
    "format": "text",                 // text | srt | vtt | json
    "include_auto_generated": true
}

# レスポンス
{
    "success": true,
    "video_id": "xxxxx",
    "title": "動画タイトル",
    "duration": 600,
    "subtitles": {
        "language": "ja",
        "auto_generated": false,
        "content": "00:00 こんにちは\n00:05 今日は...",
        // または format=json の場合
        "segments": [
            {"start": 0, "end": 5, "text": "こんにちは"},
            {"start": 5, "end": 10, "text": "今日は..."}
        ]
    }
}
```

### 6.3 動画ダウンロード (yt-dlp連携)

```http
POST /v2/media/youtube/download
{
    "url": "https://www.youtube.com/watch?v=xxxxx",
    "format": {
        "video": "best[height<=1080]",
        "audio": "bestaudio",
        "merge": true
    },
    "output": {
        "type": "file",
        "path": "/downloads/videos",
        "filename_template": "%(title)s.%(ext)s"
    },
    "options": {
        "extract_audio": false,       // 音声のみ抽出
        "add_metadata": true,
        "embed_thumbnail": true
    }
}

# レスポンス（ダウンロード開始）
{
    "success": true,
    "job_id": "dl_12345",
    "status": "started",
    "poll_url": "/v2/jobs/dl_12345"   # 統一ジョブAPI
}

# 進捗確認
GET /v2/jobs/dl_12345
{
    "job_id": "dl_12345",
    "type": "video_download",
    "status": "completed",           // pending | running | completed | failed
    "progress": {
        "percent": 100
    },
    "result": {
        "path": "/downloads/videos/動画タイトル.mp4",
        "size": 150000000,
        "duration": 600
    }
}
```

### 6.4 サイト対応状況

yt-dlpは多くのサイトに対応:

| サイト | 対応機能 |
|--------|---------|
| YouTube | 動画、字幕、プレイリスト、ライブ |
| Twitter/X | 動画、GIF |
| TikTok | 動画 |
| Vimeo | 動画 |
| ニコニコ動画 | 動画、コメント |
| bilibili | 動画 |
| その他 | 1000+サイト対応 |

### 6.5 動画分析パイプライン (FFmpeg連携)

AIが動画を分析する際、以下のデータが必要:
- サムネイル画像（代表フレーム）
- シーン変更タイミング
- 音声トラック
- 字幕データ

これらを**1回のAPIコール**で取得可能に:

```http
POST /v2/media/video/analyze
{
    "url": "https://www.youtube.com/watch?v=xxxxx",
    "analysis": {
        "keyframes": {
            "enabled": true,
            "method": "scene_change",    // scene_change | interval | iframes
            "threshold": 0.3,            // シーン変更の閾値
            "max_frames": 50,
            "format": "jpeg",
            "quality": 80
        },
        "audio": {
            "enabled": true,
            "format": "mp3",             // mp3 | wav | aac
            "bitrate": "128k"
        },
        "subtitles": {
            "enabled": true,
            "languages": ["ja", "en"],
            "format": "json"             // text | srt | vtt | json
        },
        "metadata": {
            "enabled": true              // 動画情報（長さ、解像度など）
        }
    },
    "output": {
        "file_ref": "video_analysis_123",  // 後でファイル参照用
        "base_path": "/analysis"
    }
}

# レスポンス
{
    "success": true,
    "file_ref": "video_analysis_123",
    "video_info": {
        "id": "xxxxx",
        "title": "動画タイトル",
        "duration": 600,
        "resolution": "1920x1080",
        "fps": 30
    },
    "keyframes": {
        "count": 24,
        "timestamps": [0, 15.3, 28.7, 45.2, ...],
        "files": [
            "keyframe_0000.jpg",
            "keyframe_0001.jpg",
            ...
        ]
    },
    "audio": {
        "file": "audio.mp3",
        "duration": 600,
        "size": 7200000
    },
    "subtitles": {
        "language": "ja",
        "segments": [
            {"start": 0, "end": 5, "text": "こんにちは"},
            ...
        ]
    },
    "files_url": "/v2/media/files/video_analysis_123"
}
```

#### FFmpegシーン検出コマンド（内部実装）

```bash
# シーン変更検出
ffmpeg -i input.mp4 -vf "select='gt(scene,0.3)',showinfo" -vsync 0 keyframe_%04d.jpg

# iFrame抽出
ffmpeg -i input.mp4 -vf "select='eq(pict_type,I)'" -vsync 0 iframe_%04d.jpg

# 一定間隔でフレーム抽出
ffmpeg -i input.mp4 -vf "fps=1/10" -vsync 0 frame_%04d.jpg

# 音声抽出
ffmpeg -i input.mp4 -vn -acodec libmp3lame -ab 128k audio.mp3
```

### 6.6 メディアファイル参照システム

**問題**: 大きなメディアファイルをAPIレスポンスに直接含めると:
- レスポンスサイズが巨大化
- メモリ消費
- タイムアウト

**解決**: ファイル参照IDを使ったファイルアクセス

```http
# 分析結果のファイル一覧
GET /v2/media/files/video_analysis_123
{
    "file_ref": "video_analysis_123",
    "created_at": "2026-02-05T16:00:00Z",
    "expires_at": "2026-02-06T16:00:00Z",
    "files": [
        {"name": "keyframe_0000.jpg", "size": 45000, "type": "image/jpeg"},
        {"name": "keyframe_0001.jpg", "size": 52000, "type": "image/jpeg"},
        {"name": "audio.mp3", "size": 7200000, "type": "audio/mpeg"},
        {"name": "subtitles.json", "size": 15000, "type": "application/json"}
    ],
    "total_size": 7500000
}


# 個別ファイルダウンロード
GET /v2/media/files/video_analysis_123/keyframe_0000.jpg
→ バイナリレスポンス

# ZIPでまとめてダウンロード
GET /v2/media/files/video_analysis_123?format=zip
→ video_analysis_123.zip

# バッチスクリプトからの利用例
curl -O "http://localhost:9400/v2/media/files/video_analysis_123/audio.mp3"
```

#### PowerShell/バッチからの利用

```powershell
# 動画分析リクエスト
$result = Invoke-RestMethod -Uri "http://localhost:9400/v2/media/video/analyze" `
    -Method Post -Body $jsonBody -ContentType "application/json"

$ref = $result.file_ref

# キーフレームをダウンロード
foreach ($file in $result.keyframes.files) {
    Invoke-WebRequest -Uri "http://localhost:9400/v2/media/files/$ref/$file" `
        -OutFile "frames/$file"
}

# 音声ダウンロード
Invoke-WebRequest -Uri "http://localhost:9400/v2/media/files/$ref/audio.mp3" `
    -OutFile "audio.mp3"
```

---

### 6.7 ブラウザダウンロード機能

#### 問題点

従来のスクレイピングライブラリの制限:
- ダウンロードURLが直接わからないとダウンロードできない
- JavaScript経由でのダウンロード（`blob:`, `data:`, 動的生成）が困難
- 認証付きダウンロードでCookieの引き継ぎが困難

### 解決: WebView2ネイティブダウンロード

WebView2は実際のブラウザとして動作するため、**ボタンクリックでダウンロード開始**が可能。

```http
# ダウンロードトリガー（クリックでダウンロード開始）
POST /v2/download/trigger
{
    "session": "document_portal",
    "action": {
        "type": "click",
        "selector": "#download-button"
    },
    "options": {
        "wait_for_download": true,     // ダウンロード完了まで待機
        "timeout": 300000,             // 5分タイムアウト
        "rename": "report_2026.pdf",   // ファイル名変更（オプション）
        "file_ref": "downloads_001"    // ファイル参照用
    }
}

# レスポンス（ダウンロード完了後）
{
    "success": true,
    "download": {
        "original_filename": "report.pdf",
        "saved_as": "report_2026.pdf",
        "size": 2500000,
        "mime_type": "application/pdf",
        "duration_ms": 5234
    },
    "file_ref": "/v2/media/files/downloads_001/report_2026.pdf"
}
```

### ダウンロードイベント監視

```http
# ダウンロード状態を監視（非同期）
POST /v2/download/trigger
{
    "session": "document_portal",
    "action": {"type": "click", "selector": "#download-button"},
    "options": {
        "wait_for_download": false,    // 即座に返却
        "file_ref": "downloads_001"    // ファイル参照用
    }
}

# レスポンス（即座）
{
    "success": true,
    "job_id": "dl_abc123",
    "status": "started",
    "poll_url": "/v2/jobs/dl_abc123"   # 統一ジョブAPI
}

# 進捗確認
GET /v2/jobs/dl_abc123
{
    "job_id": "dl_abc123",
    "type": "download",
    "status": "running",               // pending | running | completed | failed
    "filename": "large_file.zip",
    "progress": {
        "downloaded": 52428800,        // 50MB
        "total": 104857600,            // 100MB
        "percent": 50,
        "speed_bps": 10485760          // 10MB/s
    },
    "started_at": "2026-02-05T16:10:00Z",
    "eta_seconds": 5
}

# 完了後
{
    "job_id": "dl_abc123",
    "status": "completed",
    "result": {
        "filename": "large_file.zip",
        "file_ref": "/v2/media/files/downloads_001/large_file.zip",
        "size": 104857600
    },
    "completed_at": "2026-02-05T16:10:10Z"
}
```

### WebSocket でのリアルタイム通知

```javascript
// WebSocket接続
const ws = new WebSocket('ws://localhost:9400/v2/events');

ws.onmessage = (event) => {
    const data = JSON.parse(event.data);
    
    if (data.type === 'download_progress') {
        console.log(`${data.filename}: ${data.percent}%`);
    }
    
    if (data.type === 'download_completed') {
        console.log(`完了: ${data.file_ref}`);
    }
};
```

### WebView2 ダウンロードハンドリング（内部実装）

```rust
// WebView2のダウンロードイベント処理
webview.add_DownloadStarting(|sender, args| {
    let download = args.DownloadOperation()?;
    
    // ダウンロード先を設定
    args.SetResultFilePath(&HSTRING::from(target_path))?;
    
    // 進捗監視
    download.add_BytesReceivedChanged(|download, _| {
        let received = download.BytesReceived()?;
        let total = download.TotalBytesToReceive()?;
        // 進捗を通知...
        Ok(())
    })?;
    
    // 完了監視
    download.add_StateChanged(|download, _| {
        match download.State()? {
            COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED => {
                // 完了通知...
            }
            COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED => {
                // エラー処理...
            }
            _ => {}
        }
        Ok(())
    })?;
    
    Ok(())
})?;
```

---

### 6.8 ファイルストレージ管理

> **関連**: ファイル参照API仕様は [6.6 メディアファイル参照システム](#66-メディアファイル参照システム) を参照

#### ストレージ構造

```
$WEBVIEW_BRIDGE_DATA/
├── profiles/                    # ブラウザプロファイル（永続）
│   ├── default/
│   ├── rakuten/
│   └── google/
├── sessions/                    # セッション状態
│   └── sessions.json
├── media/                       # メディアファイル（管理対象）
│   ├── downloads_001/           # file_refごと
│   │   ├── report.pdf
│   │   └── data.xlsx
│   ├── video_analysis_123/
│   │   ├── keyframe_0001.jpg
│   │   └── audio.mp3
│   └── screenshots/
│       └── fullpage_001.png
└── cache/                       # キャッシュ（自動削除）
    └── temp/
```


### ファイルライフサイクル

```
┌─────────────────────────────────────────────────────────────────┐
│                    ファイルライフサイクル                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  [作成]                                                         │
│     │                                                           │
│     ▼                                                           │
│  ┌────────────────────┐                                        │
│  │ file_ref に紐づけ     │                                       │
│  │ TTL: 24時間 (デフォルト)│                                    │
│  └────────────────────┘                                        │
│     │                                                           │
│     ├──────────────────────────────────────┐                   │
│     │                                      │                   │
│     ▼                                      ▼                   │
│  [アクセスあり]                        [アクセスなし]           │
│  TTL延長                               │                       │
│     │                                      ▼                   │
│     │                              ┌──────────────┐            │
│     │                              │ TTL期限切れ   │            │
│     │                              └──────────────┘            │
│     │                                      │                   │
│     ▼                                      ▼                   │
│  [永続化リクエスト]                   [自動削除]               │
│  POST /v2/media/persist                                        │
│     │                                                           │
│     ▼                                                           │
│  ┌────────────────────┐                                        │
│  │ permanent フラグ設定 │                                       │
│  │ 手動削除のみ         │                                       │
│  └────────────────────┘                                        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### ストレージ管理API

```http
# ストレージ状況確認
GET /v2/storage/status
{
    "total_size": 5368709120,          // 5GB
    "used_size": 1073741824,           // 1GB
    "free_size": 4294967296,           // 4GB
    "breakdown": {
        "profiles": 524288000,         // 500MB
        "media": 536870912,            // 512MB
        "cache": 12582912              // 12MB
    },
    "file_refs": [
        {
            "ref": "downloads_001",
            "size": 52428800,
            "files_count": 3,
            "created_at": "2026-02-05T16:00:00Z",
            "expires_at": "2026-02-06T16:00:00Z",
            "permanent": false
        },
        {
            "ref": "video_analysis_123",
            "size": 104857600,
            "files_count": 25,
            "created_at": "2026-02-05T15:00:00Z",
            "expires_at": null,        // 永続化済み
            "permanent": true
        }
    ]
}

# ファイルを永続化（TTL無効化）
POST /v2/media/persist
{
    "file_ref": "downloads_001",
    "files": ["report.pdf"],           // 特定ファイルのみ、または省略で全部
    "reason": "重要レポート"           // メモ（オプション）
}

# レスポンス
{
    "success": true,
    "persisted": ["report.pdf"],
    "new_expires_at": null             // 永続化 = 期限なし
}

# TTL延長
POST /v2/media/extend
{
    "file_ref": "downloads_001",
    "extend_hours": 48                 // 48時間延長
}

# 手動削除
DELETE /v2/media/files/downloads_001
{
    "success": true,
    "deleted_files": 3,
    "freed_bytes": 52428800
}

# 期限切れファイルの即時削除（メンテナンス用）
POST /v2/storage/cleanup
{
    "success": true,
    "deleted_refs": ["temp_001", "temp_002"],
    "freed_bytes": 1073741824
}
```

### ストレージ設定

```http
# ストレージ設定
POST /v2/config/storage
{
    "base_path": "$USERPROFILE/.webview-bridge",
    "limits": {
        "max_total_size": 10737418240,  // 10GB
        "max_file_size": 1073741824,    // 1GB
        "max_session_refs": 100
    },
    "defaults": {
        "ttl_hours": 24,                // デフォルトTTL
        "auto_cleanup_interval": 3600   // 1時間ごとにクリーンアップ
    },
    "alerts": {
        "warn_at_percent": 80,          // 80%使用で警告
        "critical_at_percent": 95       // 95%で新規作成拒否
    }
}
```

環境変数:
```bash
WEBVIEW_BRIDGE_DATA_PATH=$USERPROFILE/.webview-bridge
WEBVIEW_BRIDGE_MAX_STORAGE_GB=10
WEBVIEW_BRIDGE_DEFAULT_TTL_HOURS=24
```

---

### 6.9 ダウンロード＋ファイル管理の統合例

### ユースケース: 複数PDFダウンロード

```http
# 複数ファイルをダウンロード
POST /v2/download/batch
{
    "session": "document_portal",
    "downloads": [
        {"action": {"type": "click", "selector": "#report-2024"}},
        {"action": {"type": "click", "selector": "#report-2025"}},
        {"action": {"type": "click", "selector": "#report-2026"}}
    ],
    "options": {
        "sequential": true,            // 順次ダウンロード
        "file_ref": "annual_reports",  // ファイル参照用
        "ttl_hours": 168,              // 1週間保持
        "notify_on_complete": true
    }
}

# レスポンス
{
    "success": true,
    "batch_id": "batch_001",
    "downloads": [
        {"status": "completed", "file": "report_2024.pdf", "size": 2500000},
        {"status": "completed", "file": "report_2025.pdf", "size": 2800000},
        {"status": "completed", "file": "report_2026.pdf", "size": 3100000}
    ],
    "total_size": 8400000,
    "file_ref": "annual_reports",
    "expires_at": "2026-02-12T16:00:00Z",
    "files_url": "/v2/media/files/annual_reports"
}
```

### PowerShellからの利用

```powershell
# バッチダウンロード実行
$result = Invoke-RestMethod -Uri "http://localhost:9400/v2/download/batch" `
    -Method Post -Body $jsonBody -ContentType "application/json"

# 完了後、ZIPでまとめて取得
Invoke-WebRequest -Uri "http://localhost:9400/v2/media/files/$($result.file_ref)?format=zip" `
    -OutFile "annual_reports.zip"

# 一定期間後、サーバー側で自動削除（手動不要）
```

---

## 7. SPA (Single Page Application) 対応

### 7.1 クライアントサイドレンダリング検出

```javascript
// WebView2内で実行される検出スクリプト
(function() {
    // React/Vue/Angular等のSPA検出
    const isSPA = 
        window.__REACT_DEVTOOLS_GLOBAL_HOOK__ ||
        window.__VUE__ ||
        window.ng ||
        document.querySelector('[ng-app]') ||
        document.querySelector('[data-reactroot]') ||
        document.querySelector('#app[data-v-app]');
    
    return {
        is_spa: !!isSPA,
        framework: detectFramework(),
        initial_content_loaded: document.readyState === 'complete'
    };
})();
```

### 7.2 SPA用待機戦略

```http
POST /v2/wait
{
    "session": "default",
    "strategy": "spa_ready",
    "config": {
        "framework_detection": true,
        "wait_conditions": [
            { "type": "network_idle", "threshold": 500 },
            { "type": "dom_stable", "threshold": 1000 },
            { "type": "selector_present", "selector": ".content" }
        ],
        "timeout": 15000
    }
}
```

---

## 8. スマートスクリーンショット拡張

### 8.1 固定要素自動除去

フルページスクリーンショット時の問題:
- `position: fixed` のヘッダー/フッターが各スクロール位置で重複描画
- `position: sticky` のナビゲーションが繰り返し表示

**解決**: CSSを一時的に変更して撮影

```http
POST /v2/screenshot
{
    "session": "default",
    "type": "fullpage",
    "smart_options": {
        "remove_fixed_elements": true,    // position:fixed を一時無効化
        "remove_sticky_elements": true,   // position:sticky を一時無効化
        "hide_selectors": [".cookie-banner", ".popup"],  // 追加で非表示
        "restore_after": true             // 撮影後にCSSを復元
    }
}
```

#### 内部実装（JavaScriptインジェクション）

```javascript
// 固定要素検出と一時無効化
(function() {
    const fixedElements = [];
    const stickyElements = [];
    
    // 全要素をスキャン
    document.querySelectorAll('*').forEach(el => {
        const style = window.getComputedStyle(el);
        
        if (style.position === 'fixed') {
            fixedElements.push({
                element: el,
                original: {
                    position: el.style.position,
                    top: el.style.top,
                    bottom: el.style.bottom,
                    zIndex: el.style.zIndex
                }
            });
            // 一時的にabsoluteに変更
            el.style.position = 'absolute';
            el.style.top = '0';
        }
        
        if (style.position === 'sticky') {
            stickyElements.push({
                element: el,
                original: el.style.position
            });
            el.style.position = 'relative';
        }
    });
    
    return {
        fixed_count: fixedElements.length,
        sticky_count: stickyElements.length,
        restore: function() {
            // 元のスタイルに復元
            fixedElements.forEach(item => {
                Object.assign(item.element.style, item.original);
            });
            stickyElements.forEach(item => {
                item.element.style.position = item.original;
            });
        }
    };
})();
```

### 8.2 フルページキャプチャ改善版

```
┌─────────────────────────────────────────────────────────────────┐
│              フルページスクリーンショット（改善版）              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  1. 固定要素の検出と一時無効化                                  │
│     └─ position: fixed/sticky → relative/absolute               │
│                                                                 │
│  2. ページ総高さ取得                                            │
│     └─ document.scrollingElement.scrollHeight                   │
│                                                                 │
│  3. スクロール + キャプチャループ                               │
│     ┌─────────────┐                                             │
│     │ Viewport 1  │ ← キャプチャ                               │
│     ├─────────────┤                                             │
│     │ Viewport 2  │ ← スクロール → キャプチャ                  │
│     ├─────────────┤                                             │
│     │ Viewport 3  │ ← スクロール → キャプチャ                  │
│     └─────────────┘                                             │
│                                                                 │
│  4. 画像スティッチング（Rust側で処理）                          │
│     └─ imageライブラリで縦結合                                  │
│                                                                 │
│  5. 固定要素の復元                                              │
│     └─ 元のCSSスタイルに戻す                                    │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 8.3 スクリーンショット結果のファイル参照

大きなスクリーンショットもファイル参照で取得:

```http
POST /v2/screenshot
{
    "session": "default",
    "type": "fullpage",
    "output": "file_ref",              // base64 | file_ref
    "file_ref": "screenshot_001"
}

# レスポンス
{
    "success": true,
    "file_ref": "screenshot_001",
    "dimensions": {
        "width": 1920,
        "height": 15000
    },
    "file_size": 2500000,
    "file_url": "/v2/media/files/screenshot_001/fullpage.png"
}

# ファイルダウンロード
GET /v2/media/files/screenshot_001/fullpage.png
```

---

## 9. 設計原則の補足

### 9.1 通信回数最小化

| 操作 | 従来 | WBP2 |
|------|------|------|
| ページネーション5ページ | 15回以上 | **1回** |
| 商品リスト抽出+フィルター | 10回以上 | **1回** |
| YouTube字幕取得 | N/A | **1回** |
| 画像一括収集20枚 | 21回 | **1回** |
| **動画分析（キーフレーム+音声+字幕）** | N/A | **1回** |
| **フルページSS（固定要素除去）** | 手動対応 | **1回** |

### 9.2 AIコンテキスト効率

```
従来:
  User: "楽天で商品検索して"
  AI: navigate → wait → type → click → wait → extract → ...
  → 複数ターンのやり取り、コンテキスト消費大

WBP2:
  User: "楽天で商品検索して"
  AI: execute_macro("rakuten_product_search", {keyword: "..."})
  → 1ターンで完結、結果のみ返却

動画分析:
  User: "このYouTube動画を分析して"
  AI: video_analyze(url, {keyframes: true, audio: true, subtitles: true})
  → 1ターン: キーフレーム画像 + 音声 + 字幕データすべて取得
  → AIは files_ref を使って必要なデータにアクセス
```

### 9.3 ファイル参照パターン

```
┌─────────────────────────────────────────────────────────────────┐
│                    ファイル参照パターン                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  [API Call]                                                     │
│      │                                                          │
│      ▼                                                          │
│  ┌─────────────────────────────────────────────────────┐       │
│  │ Response (軽量)                                      │       │
│  │ {                                                    │       │
│  │   "success": true,                                   │       │
│  │   "file_ref": "analysis_123",                        │       │
│  │   "summary": { ... },        ← 概要情報のみ          │       │
│  │   "files_url": "/v2/media/files/analysis_123"        │       │
│  │ }                                                    │       │
│  └─────────────────────────────────────────────────────┘       │
│      │                                                          │
│      ▼                                                          │
│  [必要に応じてファイル取得]                                     │
│  GET /v2/media/files/analysis_123/keyframe_0001.jpg             │
│  GET /v2/media/files/analysis_123/audio.mp3                     │
│  GET /v2/media/files/analysis_123?format=zip                    │
│                                                                 │
│  利点:                                                          │
│  ├── APIレスポンスが軽量                                        │
│  ├── 必要なファイルだけ取得可能                                 │
│  ├── バッチスクリプトから簡単にアクセス                         │
│  └── 大きなファイルもストリーミングで取得                       │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 10. レガシープロトコル廃止方針

### 10.1 廃止対象

WBP2への一本化に伴い、以下のプロトコルを**非推奨**とする:

| プロトコル | 状態 | 移行先 |
|-----------|------|--------|
| REST API v1 | **非推奨** | `/v2/*` に移行 |
| WebDriver (W3C) | **非推奨** | WBP2 Native API |
| CDP HTTP | **非推奨** | WBP2 + WebSocket |
| MCP (旧形式) | **非推奨** | MCP v2 (WBP2ベース) |

### 10.2 移行スケジュール

| フェーズ | 期間 | 内容 |
|---------|------|------|
| Phase A | 即時 | v1 APIに `X-Deprecated` ヘッダー追加 |
| Phase B | 1ヶ月後 | v1 API使用時にログ警告 |
| Phase C | 3ヶ月後 | v1 APIを削除（オプションで維持） |

### 10.3 WBP2統一のメリット

```
従来:                              WBP2統一後:
┌─────────────────────┐           ┌─────────────────────┐
│     REST v1         │           │                     │
├─────────────────────┤           │                     │
│     WebDriver       │           │      WBP2 API       │
├─────────────────────┤   ───→    │                     │
│     CDP             │           │   (単一設計)        │
├─────────────────────┤           │                     │
│     MCP (旧)        │           │                     │
└─────────────────────┘           └─────────────────────┘
  コード複雑、保守困難              シンプル、AIに最適化
```

---

## 11. AI-in-the-Loop 設計

### 11.1 設計思想

```
┌─────────────────────────────────────────────────────────────────┐
│                    AI-in-the-Loop アーキテクチャ                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  [従来のパターン]                                               │
│                                                                 │
│  クライアントAI                                           サーバー
│      │                                                      │
│      │ 1. ページを開いて                                    │
│      │ ─────────────────────────────────────────────────→   │
│      │ 2. 未ログイン検出                                    │
│      │ ←─────────────────────────────────────────────────   │
│      │ 3. ログインボタンをクリック                          │
│      │ ─────────────────────────────────────────────────→   │
│      │ 4. CAPTCHAが表示された...どうする？                  │
│      │ ←─────────────────────────────────────────────────   │
│      │ 5. reCAPTCHAをクリック (推測)                        │
│      │ ─────────────────────────────────────────────────→   │
│      │    ... 10往復以上、コンテキスト消費大                │
│                                                                 │
│  [AI-in-the-Loop パターン]                                      │
│                                                                 │
│  クライアントAI                 サーバーAI           WebView2   │
│      │                             │                   │        │
│      │ 1. ログインして            │                   │        │
│      │ ─────────────────────────→  │                   │        │
│      │                             │ スクショ取得      │        │
│      │                             │ ─────────────────→│        │
│      │                             │ Gemini分析        │        │
│      │                             │ ログインボタン検出│        │
│      │                             │ クリック実行      │        │
│      │                             │ ─────────────────→│        │
│      │                             │ 再スクショ        │        │
│      │                             │ CAPTCHA検出       │        │
│      │                             │ 解決または通知    │        │
│      │                             │                   │        │
│      │    {success: true}          │                   │        │
│      │ ←─────────────────────────  │                   │        │
│      │                                                          │
│      │ → 1往復で完了、コンテキスト消費最小                      │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 11.2 サーバーサイドAI設定

```http
# AI設定（サーバー起動時または動的設定）
POST /v2/config/ai
{
    "provider": "gemini",
    "api_key": "AIza...",              // 環境変数 GEMINI_API_KEY でも可
    "model": "gemini-2.0-flash",       // 高速・低コスト
    "fallback_model": "gemini-1.5-pro", // 複雑なケース用
    "settings": {
        "max_tokens": 4096,
        "temperature": 0.3,            // 低め = 安定した出力
        "vision_enabled": true         // 画像分析有効
    },
    "usage_limits": {
        "requests_per_minute": 60,
        "daily_budget_usd": 10.0       // 日次予算制限
    }
}
```

環境変数での設定:
```bash
WEBVIEW_BRIDGE_AI_PROVIDER=gemini
WEBVIEW_BRIDGE_AI_API_KEY=AIza...
WEBVIEW_BRIDGE_AI_MODEL=gemini-2.0-flash
```

---

## 12. 自動ログイン (AI-Assisted Login)

### 12.1 基本フロー

```http
POST /v2/ai/login
{
    "session": "rakuten",
    "url": "https://www.rakuten.co.jp/",
    "credentials": {
        "username": "user@example.com",
        "password": "***"              // セキュアストレージから取得推奨
    },
    "options": {
        "max_steps": 10,               // 最大ステップ数
        "timeout": 60000,              // タイムアウト
        "captcha_handling": "notify",  // notify | solve_if_simple | fail
        "two_factor": "notify"         // 2FA検出時の動作
    }
}

# レスポンス
{
    "success": true,
    "steps_taken": 4,
    "final_url": "https://www.rakuten.co.jp/mypage/",
    "auth_status": {
        "logged_in": true,
        "username_detected": "user@example.com"
    },
    "ai_log": [
        {"step": 1, "action": "detected_login_button", "selector": "#login-btn"},
        {"step": 2, "action": "clicked", "element": "ログインボタン"},
        {"step": 3, "action": "filled_form", "fields": ["username", "password"]},
        {"step": 4, "action": "submitted", "result": "success"}
    ]
}
```

### 12.2 困難なケースの処理

```http
# CAPTCHA検出時
{
    "success": false,
    "status": "captcha_required",
    "captcha_type": "recaptcha_v2",
    "screenshot_ref": "captcha_screen_001",
    "message": "CAPTCHAが検出されました。手動解決が必要です。",
    "resume_instructions": {
        "after_manual_solve": "POST /v2/ai/login/resume?session=rakuten"
    }
}

# 2FA検出時
{
    "success": false,
    "status": "2fa_required",
    "2fa_type": "sms",
    "message": "SMSコードを入力してください",
    "input_prompt": {
        "type": "code",
        "length": 6,
        "submit_to": "POST /v2/ai/login/2fa"
    }
}
```

### 12.3 認証状態の自動検出

AIが画面を見て判断:

```javascript
// サーバー内部でGeminiに送るプロンプト例
const prompt = `
この画面のスクリーンショットを分析してください:
1. ユーザーはログインしていますか？
2. ログインしている場合、ユーザー名やメールアドレスが表示されていますか？
3. ログインしていない場合、ログインボタンはどこにありますか？

JSON形式で回答:
{
  "logged_in": boolean,
  "user_info": string | null,
  "login_button": {
    "found": boolean,
    "description": string,
    "approximate_location": "top-right" | "header" | "center" | ...
  }
}
`;
```

---

## 13. AI画像評価モード

### 13.1 ユースケース

楽天市場の競合分析例:
1. 商品ページから画像20枚を収集
2. 各画像をAIが評価
3. 評価レポート + 改善用プロンプトを返却

```http
POST /v2/ai/images/analyze
{
    "session": "rakuten",
    "source": {
        "type": "page",
        "selector": ".item-image img",
        "limit": 20
    },
    "analysis": {
        "mode": "product_listing",     // product_listing | general | custom
        "aspects": [
            "visual_quality",          // 画質評価
            "composition",             // 構図
            "text_readability",        // テキストの読みやすさ
            "brand_consistency",       // ブランド一貫性
            "competitive_advantage"    // 競合優位性
        ],
        "generate_prompts": true,      // 改善用プロンプト生成
        "compare_to_best": true        // ベスト画像との比較
    }
}

# レスポンス
{
    "success": true,
    "images_analyzed": 20,
    "summary": {
        "average_score": 7.2,
        "best_image": "image_003.jpg",
        "weakest_areas": ["text_readability", "composition"],
        "recommendations": [
            "テキストのコントラストを改善してください",
            "商品を中央に配置することで視認性が向上します"
        ]
    },
    "images": [
        {
            "url": "https://...",
            "file_ref": "analysis_001/image_001.jpg",
            "scores": {
                "visual_quality": 8,
                "composition": 6,
                "text_readability": 5,
                "brand_consistency": 7,
                "competitive_advantage": 6
            },
            "analysis": "商品は見やすいが、テキストが小さく読みにくい。背景色との...",
            "improvement_prompt": "A product photography of [商品名], centered composition, clean white background, high contrast text overlay showing price and features, professional lighting..."
        },
        ...
    ],
    "comparison_report": {
        "vs_competitors": "この商品画像セットは競合と比較して...",
        "market_position": "中程度の品質。上位20%に入るには..."
    },
    "files_ref": "/v2/media/files/analysis_001"
}
```

### 13.2 カスタム評価基準

```http
POST /v2/ai/images/analyze
{
    "session": "default",
    "source": {...},
    "analysis": {
        "mode": "custom",
        "custom_prompt": "これらの商品画像を以下の観点で評価してください:\n1. 楽天市場のガイドラインに準拠しているか\n2. モバイルで見た時の視認性\n3. 購買意欲を刺激する要素があるか\n各画像に1-10点のスコアと、改善すべき点を具体的に指摘してください。"
    }
}
```

### 13.3 動的ページ解析 (AI-Assisted)

SPA/SSRで要素の表示タイミングが複雑な場合:

```http
POST /v2/ai/extract
{
    "session": "default",
    "url": "https://example.com/products",
    "goal": "商品リストを取得",
    "strategy": "ai_assisted",         // ai_assisted | selector | hybrid
    "hints": {
        "expected_items": "商品カード(画像、タイトル、価格を含む)",
        "approximate_count": "20-50件程度"
    },
    "options": {
        "wait_strategy": "ai_determined",  // AIが待機条件を判断
        "scroll_if_needed": true,
        "max_ai_attempts": 3
    }
}

# レスポンス
{
    "success": true,
    "extraction_method": "ai_vision",
    "ai_reasoning": "無限スクロールを検出。3回スクロールして45件の商品を発見。",
    "items": [
        {
            "title": "商品A",
            "price": "¥1,980",
            "image": "https://...",
            "url": "https://..."
        },
        ...
    ],
    "meta": {
        "scroll_count": 3,
        "total_items": 45,
        "page_type": "infinite_scroll"
    }
}
```

---

## 14. AI統合の設計原則

### 14.1 AI使用の判断基準

```
┌─────────────────────────────────────────────────────────────────┐
│                    AI使用の判断フロー                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  リクエスト受信                                                 │
│       │                                                         │
│       ▼                                                         │
│  ┌────────────────────┐                                        │
│  │ セレクターが明示？  │──Yes──→ セレクターベースで実行         │
│  └────────────────────┘                                        │
│       │ No                                                      │
│       ▼                                                         │
│  ┌────────────────────┐                                        │
│  │ 既知のパターン？    │──Yes──→ プリセットマクロで実行         │
│  └────────────────────┘                                        │
│       │ No                                                      │
│       ▼                                                         │
│  ┌────────────────────┐                                        │
│  │ AI設定済み？       │──No───→ エラー: AI設定が必要           │
│  └────────────────────┘                                        │
│       │ Yes                                                     │
│       ▼                                                         │
│  ┌────────────────────┐                                        │
│  │ AIモードで実行      │                                        │
│  │ (スクショ→分析→   │                                        │
│  │  アクション→確認)  │                                        │
│  └────────────────────┘                                        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 14.2 コスト最適化

```
┌─────────────────────────────────────────────────────────────────┐
│                    AI呼び出しコスト最適化                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  [レイヤー1: セレクターベース] コスト: $0                       │
│      明示的なセレクターがあれば、AIを使わない                   │
│                                                                 │
│  [レイヤー2: パターンマッチング] コスト: $0                     │
│      既知のサイト/パターンはルールベースで処理                  │
│                                                                 │
│  [レイヤー3: Gemini Flash] コスト: 低 (~$0.001/リクエスト)      │
│      画像分析、要素検出、簡単な判断                             │
│                                                                 │
│  [レイヤー4: Gemini Pro] コスト: 中 (~$0.01/リクエスト)         │
│      複雑な推論、長文生成 (フォールバック時のみ)                │
│                                                                 │
│  日次予算制限を設定して、想定外のコスト発生を防止               │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 14.3 プライバシー・セキュリティ

| 懸念事項 | 対策 |
|---------|------|
| 認証情報のAI送信 | スクリーンショットからパスワードフィールドをマスク |
| 個人情報の送信 | センシティブデータ検出→自動ぼかし |
| APIキーの管理 | 環境変数/セキュアストレージ使用 |
| ログ保存 | AIリクエスト/レスポンスの適切なログ管理 |

```javascript
// スクリーンショット送信前のマスク処理
(function() {
    // パスワードフィールドを黒塗り
    document.querySelectorAll('input[type="password"]').forEach(el => {
        el.style.background = '#000';
        el.value = '••••••••';
    });
    
    // クレジットカード番号等のマスク
    document.querySelectorAll('[autocomplete*="cc-"]').forEach(el => {
        el.style.background = '#000';
    });
})();
```

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

---

## 15. 設計整合性分析

### 15.1 発見された不整合・重複

| 問題 | 箇所 | 解決策 |
|------|------|--------|
| セッション参照の命名 | `/v2/session/acquire` で `session_id` を返すが、他APIでは `session` パラメータ | 統一: すべて `session` に |
| ファイル参照の二重定義 | 6.6メディアファイル参照と15.3ストレージ管理で類似API | 統合: `/v2/media/files/*` に一本化 |
| 進捗URL形式 | `/v2/download/status/:id` と `/v2/media/job/:id` | 統一: `/v2/jobs/:id` |
| セッション vs session_ref | ブラウザセッション と ファイルセッション参照で混乱の可能性 | 明確化: `browser_session` / `file_ref` |

### 15.2 用語統一

```
┌─────────────────────────────────────────────────────────────────┐
│                    用語定義（統一版）                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  [ブラウザ関連]                                                 │
│  ├── session: ブラウザセッション（名前付き、例: "rakuten"）     │
│  ├── profile: ブラウザプロファイル（Cookieなど永続化）          │
│  └── webview: WebView2インスタンス                              │
│                                                                 │
│  [ファイル関連]                                                 │
│  ├── file_ref: ファイル参照ID（例: "downloads_001"）            │
│  ├── file_path: ファイルへの相対パス                            │
│  └── storage: ストレージ管理全体                                │
│                                                                 │
│  [ジョブ関連]                                                   │
│  ├── job_id: 非同期ジョブのID（ダウンロード、動画分析等）       │
│  ├── status: ジョブ状態 (pending|running|completed|failed)      │
│  └── progress: 進捗情報（percent、ETA等）                       │
│                                                                 │
│  [AI関連]                                                       │
│  ├── ai_mode: AI機能を使用するかどうか                          │
│  ├── ai_log: AI判断のログ                                       │
│  └── ai_config: AI設定（APIキー、モデル等）                     │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 15.3 API命名規則（統一版）

```
/v2/session/*          - ブラウザセッション管理
/v2/navigate           - ページナビゲーション
/v2/action/*           - ブラウザアクション (click, type等)
/v2/extract            - データ抽出
/v2/wait               - 条件待機
/v2/screenshot         - スクリーンショット
/v2/macro              - マクロ実行
/v2/download/*         - ブラウザダウンロード
/v2/media/*            - メディア収集・ファイル管理
/v2/storage/*          - ストレージ管理
/v2/jobs/:id           - 非同期ジョブ状態（統一）
/v2/ai/*               - AI機能
/v2/config/*           - 設定
/v2/events             - WebSocketイベント
```

---

## 16. AI通信設計

### 16.1 通信パターン比較

```
┌─────────────────────────────────────────────────────────────────┐
│                    通信パターン比較                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  [A. REST (現在)]                                               │
│  ├── メリット                                                   │
│  │   ├── シンプル、ステートレス                                 │
│  │   ├── デバッグ容易                                           │
│  │   ├── AI (MCP/Anthropic) と自然に統合                        │
│  │   └── リトライ・エラーハンドリングが容易                     │
│  ├── デメリット                                                 │
│  │   ├── リアルタイム通知不可（ポーリング必要）                 │
│  │   ├── 長時間処理でタイムアウトの懸念                         │
│  │   └── 各リクエストにセッション情報が必要                     │
│                                                                 │
│  [B. WebSocket (補助)]                                          │
│  ├── メリット                                                   │
│  │   ├── リアルタイム通知（ダウンロード進捗等）                 │
│  │   ├── サーバープッシュ可能                                   │
│  │   └── 長時間接続維持                                         │
│  ├── デメリット                                                 │
│  │   ├── AI (LLM) の長時間接続維持は困難                        │
│  │   ├── 接続断時の再接続ロジックが必要                         │
│  │   └── ステートフルで複雑                                     │
│                                                                 │
│  [C. Webhook (コールバック)]                                    │
│  ├── メリット                                                   │
│  │   ├── 長時間処理に最適                                       │
│  │   ├── AIサーバーは接続維持不要                               │
│  │   └── 処理完了時に通知                                       │
│  ├── デメリット                                                 │
│  │   ├── コールバックURL設定が必要                              │
│  │   ├── AIエージェントがWebhook受信可能である必要              │
│  │   └── セキュリティ設計が必要                                 │
│                                                                 │
│  [D. ハイブリッド (推奨)]                                       │
│  ├── 同期処理 → REST                                            │
│  ├── 進捗監視 → ポーリング (GET /v2/jobs/:id)                   │
│  ├── リアルタイム通知 → WebSocket (オプション、人間向け)        │
│  └── 長時間処理完了通知 → Webhook (オプション)                  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 16.2 推奨アーキテクチャ

```
┌─────────────────────────────────────────────────────────────────┐
│                    AI通信アーキテクチャ（推奨）                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  [短時間処理 <30秒]                                             │
│  ├── REST同期レスポンス                                         │
│  └── 例: navigate, click, extract, screenshot                   │
│                                                                 │
│  AIエージェント ──→ POST /v2/action/click ──→ レスポンス(同期)  │
│                                                                 │
│  ───────────────────────────────────────────────────────────    │
│                                                                 │
│  [長時間処理 >30秒]                                             │
│  ├── REST非同期開始 → job_id返却                                │
│  ├── ポーリングで進捗確認 OR Webhook通知                        │
│  └── 例: download, video_analyze, paginated_extract             │
│                                                                 │
│  AIエージェント ──→ POST /v2/download/trigger                   │
│                       │                                         │
│                       ▼                                         │
│                    {job_id: "dl_001", status: "started"}        │
│                       │                                         │
│                       │ (ポーリング)                            │
│                       ▼                                         │
│                    GET /v2/jobs/dl_001                          │
│                       │                                         │
│                       ▼                                         │
│                    {status: "completed", file_ref: "..."}       │
│                                                                 │
│  ───────────────────────────────────────────────────────────    │
│                                                                 │
│  [Webhook通知 (オプション)]                                     │
│                                                                 │
│  AIエージェント ──→ POST /v2/download/trigger                   │
│                       {webhook: "https://ai-server/callback"}   │
│                       │                                         │
│                       ▼                                         │
│                    {job_id: "dl_001", status: "started"}        │
│                       │                                         │
│                       │ (処理完了後、サーバーからPOST)          │
│                       ▼                                         │
│  AIサーバー ←── POST https://ai-server/callback                 │
│                    {job_id: "dl_001", status: "completed"}      │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 16.3 Webhook設計

```http
# Webhook付きリクエスト
POST /v2/download/trigger
{
    "session": "document_portal",
    "action": {"type": "click", "selector": "#download"},
    "async": true,
    "webhook": {
        "url": "https://ai-server.example.com/webhook",
        "method": "POST",
        "headers": {
            "Authorization": "Bearer xxx",
            "X-Request-ID": "req_12345"
        },
        "events": ["completed", "failed"],  // 通知するイベント
        "retry": {
            "max_attempts": 3,
            "backoff_ms": 1000
        }
    }
}

# レスポンス（即座）
{
    "success": true,
    "job_id": "dl_001",
    "status": "started",
    "webhook_registered": true
}

# Webhookコールバック（処理完了時）
POST https://ai-server.example.com/webhook
{
    "event": "completed",
    "job_id": "dl_001",
    "timestamp": "2026-02-05T16:30:00Z",
    "result": {
        "status": "completed",
        "file_ref": "downloads_001",
        "files": [
            {"name": "report.pdf", "size": 2500000}
        ]
    },
    "signature": "sha256=..."  // リクエスト検証用
}
```

### 16.4 長時間処理の統一パターン

```http
# すべての長時間処理で共通のパターン

# 1. 処理開始（非同期）
POST /v2/{任意の長時間処理}
{
    ...,
    "async": true,              // 非同期モード
    "webhook": {...}            // オプション
}

# レスポンス
{
    "success": true,
    "job_id": "xxx",
    "status": "started",
    "poll_url": "/v2/jobs/xxx",
    "estimated_duration_ms": 30000
}

# 2. 進捗確認（ポーリング）
GET /v2/jobs/xxx
{
    "job_id": "xxx",
    "type": "download",         // download | video_analyze | batch_extract | ...
    "status": "running",        // pending | running | completed | failed | cancelled
    "progress": {
        "percent": 45,
        "current_step": "downloading",
        "eta_seconds": 15
    },
    "started_at": "2026-02-05T16:30:00Z",
    "updated_at": "2026-02-05T16:30:15Z"
}

# 3. 完了後の結果
GET /v2/jobs/xxx
{
    "job_id": "xxx",
    "status": "completed",
    "result": {
        "file_ref": "...",
        "data": {...}
    },
    "completed_at": "2026-02-05T16:30:30Z",
    "duration_ms": 30000
}

# 4. ジョブキャンセル
DELETE /v2/jobs/xxx
{
    "success": true,
    "status": "cancelled"
}
```

### 16.5 AI向け最適化

```
┌─────────────────────────────────────────────────────────────────┐
│                    AI向け通信最適化                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  [1. ポーリング間隔の推奨]                                      │
│  ├── 短時間処理: 不要（同期レスポンス待ち）                     │
│  ├── 中間: 2-5秒間隔                                            │
│  └── 長時間: 10-30秒間隔                                        │
│                                                                 │
│  [2. タイムアウト設定]                                          │
│  ├── 同期処理: 最大60秒                                         │
│  ├── 非同期処理: 開始通知は即座、完了は別途確認                 │
│  └── 推奨: estimated_duration_ms を参考に判断                   │
│                                                                 │
│  [3. バッチ処理]                                                │
│  ├── 複数操作を1リクエストにまとめる                            │
│  └── 例: POST /v2/batch [{action1}, {action2}, ...]            │
│                                                                 │
│  [4. コンテキスト効率]                                          │
│  ├── 結果は構造化JSONで返却                                     │
│  ├── 不要な情報はフィルタリング可能                             │
│  └── エラーメッセージはAIが解釈しやすい形式                     │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 16.6 バッチリクエスト

複数の操作を1リクエストにまとめる:

```http
POST /v2/batch
{
    "session": "default",
    "operations": [
        {"id": "op1", "action": "navigate", "url": "https://example.com"},
        {"id": "op2", "action": "wait", "selector": ".content", "depends_on": "op1"},
        {"id": "op3", "action": "extract", "selector": ".item", "depends_on": "op2"},
        {"id": "op4", "action": "screenshot", "depends_on": "op2"}
    ],
    "stop_on_error": true
}

# レスポンス
{
    "success": true,
    "results": {
        "op1": {"success": true, "url": "https://example.com"},
        "op2": {"success": true, "found": true, "elapsed_ms": 234},
        "op3": {"success": true, "items": [...]},
        "op4": {"success": true, "image": "base64..."}
    },
    "total_elapsed_ms": 2500
}
```

---

## 17. WebSocket イベント設計

### 17.1 接続・購読

```javascript
// WebSocket接続
const ws = new WebSocket('ws://localhost:9400/v2/events');

ws.onopen = () => {
    // 特定のイベントを購読
    ws.send(JSON.stringify({
        type: 'subscribe',
        events: ['download_progress', 'job_completed', 'session_event'],
        session_filter: ['rakuten', 'default']  // 特定セッションのみ
    }));
};

ws.onmessage = (event) => {
    const data = JSON.parse(event.data);
    switch (data.type) {
        case 'download_progress':
            console.log(`${data.filename}: ${data.percent}%`);
            break;
        case 'job_completed':
            console.log(`ジョブ完了: ${data.job_id}`);
            break;
        case 'session_event':
            console.log(`セッション: ${data.session} - ${data.event}`);
            break;
    }
};
```

### 17.2 イベント種別

| イベント | 説明 | ペイロード例 |
|---------|------|-------------|
| `download_progress` | DL進捗 | `{filename, percent, speed_bps}` |
| `download_completed` | DL完了 | `{file_ref, size}` |
| `job_started` | ジョブ開始 | `{job_id, type}` |
| `job_progress` | ジョブ進捗 | `{job_id, percent, current_step}` |
| `job_completed` | ジョブ完了 | `{job_id, result}` |
| `job_failed` | ジョブ失敗 | `{job_id, error}` |
| `session_created` | セッション作成 | `{session, profile}` |
| `session_closed` | セッション終了 | `{session, reason}` |
| `navigation` | ページ遷移 | `{session, url}` |
| `ai_action` | AI操作ログ | `{session, action, result}` |

### 17.3 AIエージェントでのWebSocket使用

```
┌─────────────────────────────────────────────────────────────────┐
│                    AIエージェントとWebSocket                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  [課題]                                                         │
│  ├── LLMは通常、HTTPリクエスト/レスポンスモデル                 │
│  ├── 長時間のWebSocket接続維持は困難                            │
│  └── MCPなどのプロトコルはREST前提                              │
│                                                                 │
│  [推奨アプローチ]                                               │
│  ├── AIエージェント自体はRESTを使用                             │
│  │   └── ポーリングまたはWebhookで非同期結果を取得             │
│  ├── WebSocketは人間オペレーターまたは監視用                    │
│  │   └── ダッシュボード、進捗表示、デバッグ                     │
│  └── AIラッパーアプリケーションがWebSocketを使用可能            │
│      └── AI ← REST → ラッパー ← WebSocket → WBP2               │
│                                                                 │
│  [MCP Tool定義での非同期対応]                                   │
│  ├── 同期Toolは通常のTool呼び出し                               │
│  ├── 非同期Toolはjob_idを返し、別のToolで結果取得               │
│  └── 例:                                                        │
│      Tool: start_download → returns job_id                      │
│      Tool: check_job_status → returns status/result             │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 18. 設計整合性チェックリスト

### 18.1 解決済み

- [x] セッション命名統一 (`session` パラメータ)
- [x] ファイル参照API統一 (`/v2/media/files/*`)
- [x] ジョブ管理API統一 (`/v2/jobs/:id`)
- [x] 用語定義の明確化
- [x] 通信パターンの選択基準明確化

### 18.2 要検討 → 解決済み

- [x] Phase番号の再整理（機能グループ化）✅
- [x] MCP Tool定義の具体化 ✅ → 18.4参照
- [x] エラーコード体系の統一 ✅ → 18.5参照
- [ ] レート制限設計 (Phase 7以降で検討)
- [ ] 認証・認可設計 (Phase 7以降で検討)

### 18.4 MCP Tool定義 (WBP2対応版)

```json
{
  "tools": [
    {
      "name": "session_acquire",
      "description": "名前付きセッションを取得または作成",
      "inputSchema": {
        "type": "object",
        "properties": {
          "name": {"type": "string", "description": "セッション名 (例: rakuten, google)"},
          "profile": {"type": "string", "description": "プロファイル名 (オプション)"},
          "reuse": {"type": "boolean", "default": true},
          "headless": {"type": "boolean", "default": false}
        },
        "required": ["name"]
      }
    },
    {
      "name": "navigate",
      "description": "URLに移動",
      "inputSchema": {
        "type": "object",
        "properties": {
          "session": {"type": "string"},
          "url": {"type": "string"},
          "wait_until": {"type": "string", "enum": ["load", "domcontentloaded", "networkidle"]}
        },
        "required": ["session", "url"]
      }
    },
    {
      "name": "wait",
      "description": "条件が満たされるまで待機",
      "inputSchema": {
        "type": "object",
        "properties": {
          "session": {"type": "string"},
          "selector": {"type": "string"},
          "condition": {"type": "string", "enum": ["present", "visible", "stable", "text_contains"]},
          "timeout": {"type": "integer", "default": 10000},
          "extract": {"type": "array", "items": {"type": "string"}}
        },
        "required": ["session", "selector"]
      }
    },
    {
      "name": "extract",
      "description": "ページからデータを抽出",
      "inputSchema": {
        "type": "object",
        "properties": {
          "session": {"type": "string"},
          "selector": {"type": "string"},
          "fields": {"type": "array", "items": {"type": "string"}},
          "multiple": {"type": "boolean", "default": false}
        },
        "required": ["session", "selector"]
      }
    },
    {
      "name": "screenshot",
      "description": "スクリーンショットを撮影",
      "inputSchema": {
        "type": "object",
        "properties": {
          "session": {"type": "string"},
          "type": {"type": "string", "enum": ["viewport", "fullpage", "element"]},
          "selector": {"type": "string"},
          "format": {"type": "string", "enum": ["png", "jpeg", "webp"]}
        },
        "required": ["session"]
      }
    },
    {
      "name": "macro",
      "description": "プリセットまたはカスタムマクロを実行",
      "inputSchema": {
        "type": "object",
        "properties": {
          "session": {"type": "string"},
          "macro": {"type": "string", "description": "マクロ名 (extract_list, paginated_extract等)"},
          "config": {"type": "object"}
        },
        "required": ["session", "macro", "config"]
      }
    },
    {
      "name": "job_status",
      "description": "非同期ジョブの状態を確認",
      "inputSchema": {
        "type": "object",
        "properties": {
          "job_id": {"type": "string"}
        },
        "required": ["job_id"]
      }
    }
  ]
}
```

### 18.5 エラーコード体系

| コード | 名前 | 説明 | HTTP |
|--------|------|------|------|
| `WBP2_001` | `SESSION_NOT_FOUND` | 指定セッションが存在しない | 404 |
| `WBP2_002` | `SESSION_BUSY` | セッションが他の操作で使用中 | 409 |
| `WBP2_003` | `SESSION_CLOSED` | セッションが既に閉じている | 410 |
| `WBP2_010` | `SELECTOR_NOT_FOUND` | セレクターに一致する要素がない | 404 |
| `WBP2_011` | `SELECTOR_TIMEOUT` | 待機タイムアウト | 408 |
| `WBP2_012` | `ELEMENT_NOT_VISIBLE` | 要素が表示されていない | 400 |
| `WBP2_020` | `NAVIGATION_FAILED` | ナビゲーション失敗 | 502 |
| `WBP2_021` | `NAVIGATION_TIMEOUT` | ナビゲーションタイムアウト | 408 |
| `WBP2_030` | `SCRIPT_ERROR` | JavaScript実行エラー | 400 |
| `WBP2_031` | `MACRO_NOT_FOUND` | 指定マクロが存在しない | 404 |
| `WBP2_040` | `JOB_NOT_FOUND` | 指定ジョブが存在しない | 404 |
| `WBP2_041` | `JOB_FAILED` | ジョブ実行失敗 | 500 |
| `WBP2_042` | `JOB_CANCELLED` | ジョブがキャンセルされた | 410 |
| `WBP2_050` | `FILE_NOT_FOUND` | ファイルが存在しない | 404 |
| `WBP2_051` | `FILE_EXPIRED` | ファイルTTL切れ | 410 |
| `WBP2_060` | `AI_NOT_CONFIGURED` | AI設定が未構成 | 400 |
| `WBP2_061` | `AI_QUOTA_EXCEEDED` | AI予算超過 | 429 |
| `WBP2_070` | `DOWNLOAD_FAILED` | ダウンロード失敗 | 500 |
| `WBP2_080` | `STORAGE_FULL` | ストレージ容量超過 | 507 |
| `WBP2_090` | `INVALID_REQUEST` | リクエスト形式不正 | 400 |
| `WBP2_091` | `MISSING_PARAMETER` | 必須パラメータ不足 | 400 |
| `WBP2_099` | `INTERNAL_ERROR` | 内部エラー | 500 |

#### エラーレスポンス形式

```json
{
  "success": false,
  "error": {
    "code": "WBP2_011",
    "name": "SELECTOR_TIMEOUT",
    "message": "セレクター '.product-list' が10秒以内に見つかりませんでした",
    "details": {
      "selector": ".product-list",
      "timeout_ms": 10000,
      "elapsed_ms": 10023
    },
    "suggestion": "セレクターを確認するか、タイムアウトを延長してください"
  }
}
```

### 18.3 設計原則の再確認

| 原則 | 設計での対応 | 状態 |
|------|-------------|------|
| 宣言的ゴール | `/v2/goal`, マクロ, AI抽出 | ✅ 整合 |
| イベント駆動 | MutationObserver, WebSocket | ✅ 整合 |
| セッション永続化 | sessions.json, 名前付きセッション | ✅ 整合 |
| スマート待機 | `/v2/wait`, SPA対応 | ✅ 整合 |
| エラー自己回復 | AI-in-the-Loop, 自動リトライ | ✅ 整合 |
| 通信効率 | マクロ, バッチ, 非同期ジョブ | ✅ 整合 |
| ファイル管理 | TTL, 自動削除, 永続化 | ✅ 整合 |

