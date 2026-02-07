# AI エージェント視点での使いやすさ改善計画

## 概要
WebView Bridge Protocol V2 を AI エージェント (Claude/Gemini等) が効率的に使用できるように改善する計画書。

## 現状の課題と改善項目

### 1. ⚡ ページロード完了の明確な通知 [優先度: 高]

**現状の問題:**
- `navigate` 後にページが完全にロードされたかどうか不明
- AI側で `Start-Sleep` などの固定待機が必要
- ページによってロード時間が大きく異なる

**改善案:**
```json
// POST /v2/sessions/{id}/navigate のレスポンス
{
  "success": true,
  "load_state": "complete",  // "loading" | "interactive" | "complete"
  "load_time_ms": 1234,
  "url": "https://example.com"
}
```

**実装方針:**
- WebView2 の `NavigationCompleted` イベントを待機
- オプションで `wait_for_load: true` パラメータを追加
- タイムアウト設定可能に

---

### 2. 📸 ビジュアルスクリーンショット [優先度: 高]

**現状の問題:**
- Canvas ベースの実装はテキストのみ描画
- 実際のページのビジュアルが取得できない
- AI がページの視覚的状態を確認できない

**改善案:**
WebView2 の `CapturePreview` API を使用したネイティブキャプチャ

```rust
// WebView2 CapturePreview の使用
unsafe {
    webview.CapturePreview(
        COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
        stream,
        handler
    )?;
}
```

**レスポンス:**
```json
{
  "format": "png",
  "data": "<base64>",
  "width": 1280,
  "height": 900,
  "capture_type": "visual"  // "visual" | "canvas_text"
}
```

---

### 3. 🔄 統一されたスクリプト実行パス [優先度: 高]

**現状の問題:**
- `execute` と `screenshot` で異なるコードパスを使用
- 同じスクリプトでも結果が異なる可能性
- デバッグが困難

**改善案:**
すべてのスクリプト実行を単一の内部関数に統合

```rust
// core/mod.rs の統一パターン
impl SessionCommand {
    fn execute_any_script(webview: &WebViewInstance, script: &str) -> Result<String, String> {
        webview.execute_script(script, Uuid::new_v4().to_string())
            .map_err(|e| format!("{:?}", e))
    }
}
```

---

### 4. 📋 詳細なエラーレスポンス [優先度: 中]

**現状の問題:**
- `{}` が返された時に原因が分からない
- JavaScript エラーの詳細が欠落
- ネットワークエラーとスクリプトエラーの区別が困難

**改善案:**
```json
// エラーレスポンス
{
  "success": false,
  "error": {
    "code": "SCRIPT_EXECUTION_FAILED",
    "message": "JavaScript execution failed",
    "details": {
      "script_preview": "(function()...",
      "js_error": "TypeError: Cannot read property 'x' of null",
      "stack_trace": "at line 5..."
    }
  }
}
```

---

### 5. ✅ セッション状態の詳細取得 [優先度: 中]

**現状の問題:**
- WebView が ready かどうか判断しにくい
- ナビゲーション中かどうか不明
- 最後のエラー状態が取得できない

**改善案:**
```json
// GET /v2/sessions/{id}/status
{
  "id": "uuid",
  "state": "ready",           // "initializing" | "ready" | "navigating" | "error"
  "webview_ready": true,
  "current_url": "https://...",
  "page_title": "Example",
  "last_navigation": {
    "url": "https://...",
    "status": "complete",
    "load_time_ms": 1234
  },
  "last_error": null
}
```

---

### 6. 🎯 セレクター待機の改善 [優先度: 中]

**現状の問題:**
- `wait_for_selector` のタイムアウト処理が不明確
- ポーリング間隔が固定

**改善案:**
```json
// POST /v2/sessions/{id}/wait
{
  "selector": "#content",
  "timeout_ms": 10000,
  "poll_interval_ms": 100,
  "condition": "visible"  // "exists" | "visible" | "hidden" | "text_contains"
}

// レスポンス
{
  "found": true,
  "waited_ms": 450,
  "element_info": {
    "tag": "div",
    "visible": true,
    "text_preview": "Hello..."
  }
}
```

---

## 実装優先順位

| 順位 | 項目 | 理由 |
|-----|------|------|
| 1 | ビジュアルスクリーンショット | AI がページを「見る」ために必須 |
| 2 | ページロード完了通知 | 安定した自動化の基盤 |
| 3 | 統一スクリプト実行パス | バグ防止・保守性向上 |
| 4 | 詳細エラーレスポンス | デバッグ効率化 |
| 5 | セッション状態詳細 | 堅牢性向上 |
| 6 | セレクター待機改善 | 高度な自動化対応 |

---

## 実装計画

### Phase 1: コア機能修正 (今日中)
- [ ] CapturePreview ベースのビジュアルスクリーンショット実装
- [ ] 既存の canvas ベースはフォールバックとして維持
- [ ] スクリプト実行パスの統一

### Phase 2: 安定性向上 (次回)
- [ ] NavigationCompleted イベントの適切な待機
- [ ] 詳細なエラーレスポンス構造
- [ ] セッション状態管理強化

### Phase 3: 高度な機能 (将来)
- [ ] 条件付き待機 API
- [ ] ページ変更検知
- [ ] パフォーマンスメトリクス

---

## 次のアクション

1. WebView2 の `CapturePreview` API を調査・実装
2. スクリーンショット機能のテスト
3. PROTOCOL_V2.md の更新
