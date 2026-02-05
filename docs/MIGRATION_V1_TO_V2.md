# WBP2 Migration Guide: v1 → v2

> WebView Bridge Protocol v2 移行ガイド

## 概要

WBP2 v1 API は非推奨となり、将来のバージョンで削除される予定です。
このガイドでは v1 から v2 への移行方法を説明します。

## 変更の概要

| 項目 | v1 | v2 |
|------|----|----|
| セッション管理 | UUID ベース | 名前付きセッション |
| API プレフィックス | `/` | `/v2/` |
| 待機処理 | ポーリング | イベント駆動 |
| エラー形式 | 独自形式 | 構造化エラーコード |
| WebSocket | なし | リアルタイム通知 |

## API マッピング

### セッション管理

| v1 API | v2 API | 備考 |
|--------|--------|------|
| `POST /create` | `POST /v2/session/acquire` | セッション名を指定 |
| `DELETE /close/:id` | `POST /v2/session/release` | リリースとデストロイを分離 |
| `GET /status/:id` | `GET /v2/session/:name` | 詳細なステータス情報 |

### ナビゲーション

| v1 API | v2 API | 備考 |
|--------|--------|------|
| `POST /navigate/:id` | `POST /v2/goal` | type: "navigate" |

### JavaScript 実行

| v1 API | v2 API | 備考 |
|--------|--------|------|
| `POST /execute/:id` | `POST /v2/macro` | マクロエンジン経由 |

### 待機処理

| v1 API | v2 API | 備考 |
|--------|--------|------|
| `POST /wait/:id` | `POST /v2/wait` | イベント駆動対応 |

### スクリーンショット

| v1 API | v2 API | 備考 |
|--------|--------|------|
| `GET /screenshot/:id` | `POST /v2/screenshot` | デバイスエミュレーション対応 |

### スナップショット

| v1 API | v2 API | 備考 |
|--------|--------|------|
| `GET /snapshot/:id` | `POST /v2/goal` | type: "extract" |

### Cookie 管理

| v1 API | v2 API | 備考 |
|--------|--------|------|
| `GET /cookies/:id` | - | v2 では別途設計予定 |
| `POST /cookies/:id` | - | v2 では別途設計予定 |

## コード例

### v1: セッション作成
```rust
// v1
let response = client
    .post("http://localhost:9222/create")
    .json(&json!({ "profile": "default" }))
    .send()
    .await?;

let session: Value = response.json().await?;
let session_id = session["session_id"].as_str().unwrap();
```

### v2: セッション取得
```rust
// v2
let response = client
    .post("http://localhost:9222/v2/session/acquire")
    .json(&json!({
        "name": "my_session",
        "profile": "default",
        "timeout_ms": 30000
    }))
    .send()
    .await?;

let session: Value = response.json().await?;
assert!(session["success"].as_bool().unwrap());
```

### v1: 待機処理
```rust
// v1: ポーリングが必要
let response = client
    .post(&format!("http://localhost:9222/wait/{}", session_id))
    .json(&json!({
        "selector": "#content",
        "timeout": 10000
    }))
    .send()
    .await?;
```

### v2: 待機処理
```rust
// v2: イベント駆動
let response = client
    .post("http://localhost:9222/v2/wait")
    .json(&json!({
        "session": "my_session",
        "type": "element",
        "selector": "#content",
        "event": "visible",
        "timeout_ms": 10000
    }))
    .send()
    .await?;
```

## WebSocket 活用

v2 では WebSocket を使用してリアルタイム通知を受信できます：

```javascript
const ws = new WebSocket('ws://localhost:9222/v2/ws');

ws.onopen = () => {
    ws.send(JSON.stringify({
        type: 'subscribe',
        session: 'my_session'
    }));
};

ws.onmessage = (event) => {
    const data = JSON.parse(event.data);
    switch (data.type) {
        case 'dom_change':
            console.log('DOM changed:', data.payload);
            break;
        case 'navigation':
            console.log('Navigation:', data.payload);
            break;
        case 'error':
            console.error('Error:', data.payload);
            break;
    }
};
```

## エラーハンドリング

### v1 エラー形式
```json
{
    "error": "Session not found"
}
```

### v2 エラー形式
```json
{
    "success": false,
    "error": {
        "code": "WBP2_001",
        "name": "SESSION_NOT_FOUND",
        "message": "Session 'my_session' not found"
    }
}
```

## エラーコード一覧

| コード | 名前 | 説明 |
|--------|------|------|
| WBP2_001 | SESSION_NOT_FOUND | セッションが見つからない |
| WBP2_002 | SESSION_BUSY | セッションが使用中 |
| WBP2_003 | SESSION_TIMEOUT | セッション取得タイムアウト |
| WBP2_010 | ELEMENT_NOT_FOUND | 要素が見つからない |
| WBP2_011 | WAIT_TIMEOUT | 待機タイムアウト |
| WBP2_020 | SCRIPT_ERROR | スクリプト実行エラー |
| WBP2_080 | MACRO_NOT_FOUND | マクロが見つからない |
| WBP2_090 | INVALID_REQUEST | 無効なリクエスト |
| WBP2_099 | INTERNAL_ERROR | 内部エラー |

## 移行チェックリスト

- [ ] API エンドポイントを `/v2/` プレフィックスに更新
- [ ] セッション管理を名前付きセッションに変更
- [ ] 待機処理を イベント駆動 API に移行
- [ ] エラーハンドリングを構造化エラーに対応
- [ ] WebSocket 通知の活用を検討
- [ ] v1 API の使用箇所を特定し置換

## 非推奨スケジュール

| 日付 | アクション |
|------|------------|
| 現在 | v1 API に `X-Deprecated` ヘッダー追加 |
| TBD | v1 API の新規使用非推奨 |
| TBD | v1 API 削除 |

## お問い合わせ

移行中に問題が発生した場合は、GitHub Issues でお知らせください。
