# WebView Bridge Protocol v2 シンプル化計画

## 現状の問題点

### 1. 待機地獄
```powershell
# 現状: 手動で待機が必要
$r = Invoke-RestMethod /create ...
Start-Sleep 5  # ← なぜ必要？いつまで待てばいい？
Invoke-RestMethod /navigate ...
Start-Sleep 3  # ← またか...
Invoke-RestMethod /screenshot ...
```

**問題**: 
- 使う側が「どれくらい待てばいいか」わからない
- 環境・ページによって待機時間が変わる
- エラーチェックだらけのコードになる

### 2. V1/V2 二重管理
- V1: UUID ベースのセッションID
- V2: 名前ベースのセッション
- 内部でV1のIDを参照している
- どっちを使えばいいか混乱

---

## 解決策: 「待機不要API」の実装

### 基本原則
**すべての操作は完了するまで戻らない（同期的）**

```powershell
# 理想: ワンライナーで完結
$r = Invoke-RestMethod -Uri "/v2/session/acquire" -Body '{"name":"test","url":"https://google.com"}'
# ↑ これだけでセッション作成+ナビゲーション+ロード完了待ちが終わる

$ss = Invoke-RestMethod -Uri "/v2/screenshot" -Body '{"session":"test"}'
# ↑ 確実に画像が返ってくる
```

### 実装変更

#### 1. セッション取得 = 即座に ready
```rust
// POST /v2/session/acquire
{
    "name": "my_session",
    "url": "https://example.com",   // オプション: 初期URL
    "wait_until": "load"            // "domready" | "load" | "networkidle"
}

// レスポンス: セッションが完全に ready になってから返す
{
    "success": true,
    "session": "my_session",
    "ready": true,                  // 常に true で返す
    "current_url": "https://example.com",
    "title": "Example Domain"
}
```

#### 2. ナビゲーション = ロード完了まで待機
```rust
// POST /v2/navigate (新規追加)
{
    "session": "my_session",
    "url": "https://google.com",
    "wait_until": "load"            // デフォルト: "load"
}

// レスポンス: ページが完全にロードされてから返す
{
    "success": true,
    "url": "https://google.com",
    "title": "Google",
    "load_time_ms": 1234
}
```

#### 3. スクリーンショット = 確実に画像を返す
```rust
// POST /v2/screenshot
{
    "session": "my_session",
    "wait_for_images": true         // デフォルト: true
}

// レスポンス: 画像が確実に含まれる
{
    "success": true,
    "image": "<base64>",
    "width": 1280,
    "height": 900
}
```

---

## 移行計画

### Phase 1: V2 API の完全化 (今日)
1. `/v2/navigate` エンドポイント追加
2. セッション acquire 時に `url` オプションを追加
3. NavigationCompleted イベント待機を実装
4. すべての操作を同期的に

### Phase 2: V1 API 非推奨化 (次回)
1. V1 API に deprecation warning を追加
2. ドキュメントを V2 のみに
3. V1 → V2 移行スクリプト作成

### Phase 3: V1 API 削除 (将来)
1. V1 コードの完全削除
2. `api.rs` を archive
3. V2 専用としてリリース

---

## 新しい API 設計 (V2 Only)

### セッション管理
| Method | Path | 説明 |
|--------|------|------|
| POST | `/v2/session/acquire` | セッション取得 (なければ作成) |
| POST | `/v2/session/release` | セッション解放 |
| DELETE | `/v2/session/:name` | セッション破棄 |
| GET | `/v2/session/list` | セッション一覧 |
| GET | `/v2/session/:name` | セッション詳細 |

### ナビゲーション (新規)
| Method | Path | 説明 |
|--------|------|------|
| POST | `/v2/navigate` | ページ遷移 (完了まで待機) |
| POST | `/v2/click` | 要素クリック |
| POST | `/v2/type` | テキスト入力 |
| POST | `/v2/scroll` | スクロール |

### コンテンツ取得
| Method | Path | 説明 |
|--------|------|------|
| POST | `/v2/screenshot` | スクリーンショット |
| POST | `/v2/snapshot` | DOM/HTML/テキスト取得 |
| POST | `/v2/execute` | JavaScript 実行 |

### 待機 (オプション)
| Method | Path | 説明 |
|--------|------|------|
| POST | `/v2/wait` | 条件待機 (セレクタ、テキスト等) |

---

## 実装開始

上記の設計に基づき、以下の順序で実装:

1. [ ] `/v2/navigate` エンドポイント追加
2. [ ] NavigationCompleted 待機の実装
3. [ ] `/v2/session/acquire` に `url` パラメータ追加  
4. [ ] `/v2/click`, `/v2/type`, `/v2/scroll` 追加
5. [ ] V1 API に deprecation warning 追加
6. [ ] V1 API のルーティング削除
