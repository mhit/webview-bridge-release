# AI-First WebView Bridge 実装計画

> 作成日: 2026-02-06
> 目的: AIエージェント（Antigravity等）がスクリプトを書かずに直接WebView Bridgeを使えるようにする

---

## 現状の問題

### 1. AIが直接使えない
- REST APIは動作するが、AIはPowerShellスクリプトを書く必要がある
- MCPサーバーはあるが、Antigravityから認識されない
- 「AIファースト」と謳いながら、実際はヒューマンファースト

### 2. PROTOCOL_V2.mdと実装のギャップ
| 機能 | 定義 | 実装 |
|------|------|------|
| フルページスクリーンショット | ✅ 定義済み | ❌ 未実装 |
| デバイスエミュレーション | ✅ 定義済み | ❌ 未実装 |
| Goal API | ✅ 定義済み | ⚠️ 部分実装 |
| スマート待機 | ✅ 定義済み | ⚠️ 部分実装 |
| MCP統合 | ✅ 定義済み | ⚠️ REST版のみ |

### 3. MCPの問題
- 現在: REST/HTTP経由の独自実装
- 必要: stdio/SSE経由の標準MCP

---

## 実装計画

### Phase 1: MCP Standard対応 (優先度: 最高)

#### 1.1 stdio-based MCP Server
Antigravityや他のMCPクライアントが標準的に接続できる形式。

```
webview-bridge-rust.exe --mcp-stdio
```

これにより、mcp_config.jsonで以下のように設定可能:
```json
{
  "mcpServers": {
    "webview-bridge": {
      "command": "C:\\path\\to\\webview-bridge-rust.exe",
      "args": ["--mcp-stdio"]
    }
  }
}
```

#### 1.2 高機能MCPツール定義
```
Tools:
├── browse(url, session?)          # URLに移動（セッション自動管理）
├── screenshot(session?, type?)    # スクリーンショット取得
├── click(selector, session?)      # 要素をクリック
├── type(selector, text, session?) # テキスト入力
├── extract(selector, session?)    # データ抽出
├── scroll(direction, session?)    # スクロール
├── wait(condition, session?)      # 条件待機
├── search_rakuten(query)          # 楽天検索（ワンショット）
├── get_product_list(store_url)    # 商品リスト取得（ワンショット）
└── execute(script, session?)      # JavaScript実行

Resources:
├── browser://current/html         # 現在のページHTML
├── browser://current/text         # 現在のページテキスト
├── browser://current/screenshot   # 現在のスクリーンショット
└── browser://sessions             # セッション一覧
```

### Phase 2: ワンショット操作 (優先度: 高)

AIが1回の呼び出しで完結できる高レベル操作。

#### 2.1 search_site ツール
```json
{
  "name": "search_site",
  "arguments": {
    "site": "rakuten",
    "query": "AdamasOcta",
    "max_results": 20
  }
}
```
→ 自動的にセッション作成、検索、結果抽出、返却

#### 2.2 get_product_list ツール
```json
{
  "name": "get_product_list", 
  "arguments": {
    "store_url": "https://www.rakuten.co.jp/adamasocta/",
    "include_sku": true
  }
}
```
→ 店舗ページにアクセス、商品一覧を構造化データで返却

### Phase 3: ビジュアルスクリーンショット (優先度: 高)

現在のCanvas描画方式からWebView2ネイティブキャプチャへ。

#### 3.1 CapturePreview API実装
```rust
// WebView2のCapturePreviewAsync使用
controller.CoreWebView2().CapturePreviewAsync(
    CoreWebView2CapturePreviewImageFormat::Png,
    stream
)?;
```

#### 3.2 フルページ対応
- スクロール+スティッチング方式
- または CDP Page.captureScreenshot

### Phase 4: エラー自己回復 (優先度: 中)

#### 4.1 リトライロジック
- ネットワークエラー: 自動リトライ (3回)
- セレクター見つからない: AIに代替案を提案
- ページロード失敗: 待機時間延長して再試行

#### 4.2 状態報告
```json
{
  "success": false,
  "error": "element_not_found",
  "suggestion": "Try selector '.product-card' instead of '.item'",
  "page_state": {
    "url": "https://...",
    "title": "...",
    "has_content": true
  }
}
```

---

## 実装順序

```
Week 1:
├── [x] V1 API削除
├── [ ] stdio MCP実装
├── [ ] mcp_config.json連携確認
└── [ ] browse + screenshot ツール動作確認

Week 2:
├── [ ] 高レベルツール追加 (search_site, get_product_list)
├── [ ] ビジュアルスクリーンショット実装
└── [ ] セッション自動管理

Week 3:
├── [ ] フルページスクリーンショット
├── [ ] エラー自己回復
└── [ ] ドキュメント更新
```

---

## 成功基準

AIエージェントが以下のタスクをスクリプトなしで完了できること:

1. **「楽天でAdamasOctaを検索してSKUリストを作って」**
   - browse → search_site → extract → 返却

2. **「このページのスクリーンショットを撮って」**
   - screenshot → base64画像を返却

3. **「ログインして最新のお知らせを取得して」**
   - browse → wait → extract → 返却

---

## 次のアクション

1. **今すぐ**: stdio MCP対応の実装を開始
2. **確認**: Antigravityのmcp_config.json形式を調査
3. **テスト**: 最小限のstdio MCPでAntigravityから呼び出せることを確認
