# WebView Bridge - 開発担当AI用サマリー

## 概要
WindowsネイティブのWebView2操作ブリッジをRust + Axumで構築。
OpenClaw Assistant（Sam）がWebスクレイピングやWebアクセスを行うためのHTTP APIを提供する。

---

## プロジェクト情報

### 場所
- **WSL側（開発）:** `/home/mhit/webview-bridge`
- **Windows側（実行）:** `C:\Users\mhit\AppData\Local\Temp\webview-bridge-build`

### 現状
✅ 技術的到達点
- 依存関係固定: `windows 0.39.0` + `webview2-com 0.19.1`
- アーキテクチャ: メインスレッドSTA + Axum (Tokio Runtime)
- ビルド成功、起動成功
- Action API（click/type/press）実装済み

❌ 課題
- ポート9400に応答なし（Axumサーバー状態不明）
- 真っ白画面（窓は出るが中身が描画されない）
- リサイズ追従未実装

---

## 提供資料

### 1. TEST_CASES.md
**場所:** `/home/mhit/webview-bridge/TEST_CASES.md`

**内容:** Samが通常業務で使用するWebアクセス操作のテストケース（10ケース）

**主なテストケース:**
1. UC-01: X（Twitter）エゴサーチ
2. UC-02: Google Shopping価格調査
3. UC-03: Amazon商品確認
4. UC-04: Rakuten RMS注文管理
5. UC-05: Yahooショッピング商品確認
6. UC-06: 楽天ブックスレビュークロール
7. UC-07: ECサイト広告確認
8. UC-08: Adamas公式サイト巡回
9. UC-09: ターゲットサイトクロール
10. UC-10: ログイン認証

**各テストケースに含まれる情報:**
- 目的・前提条件
- 操作手順（JSON形式）
- 期待結果
- テストデータ

---

### 2. IMPLEMENTATION_REFERENCE.md
**場所:** `/home/mhit/webview-bridge/IMPLEMENTATION_REFERENCE.md`

**内容:** Samが現在運用しているPuppeteerスクリプトの実装パターン

**主な参考スクリプト:**
1. egosearch-puppeteer.mjs - Xエゴサーチ
2. scrape-amazon-reviews.mjs - Amazonレビュースクレイピング
3. x-auto-login.mjs - X自動ログイン
4. browser-pool.mjs - ブラウザープール管理

**API設計のポイント:**
- Navigate, Wait, Click, Type, Extract, Screenshot, Evaluate の7つの基本操作
- セレクター戦略: `data-testid` > id > class > タグ
- エラーハンドリング: タイムアウト・要素不在に対応
- セッション管理: Cookie/LocalStorageの永続化

---

## 実装要件

### 基本操作API

#### 1. Navigate
```
POST /navigate
{
  "url": "https://example.com",
  "waitFor": "networkidle" | "selector:xxx" | "timeout:5000",
  "timeout": 30000
}
```

#### 2. Wait for Selector
```
POST /wait
{
  "selector": "article[data-testid='tweet']",
  "timeout": 15000,
  "ignoreErrors": true
}
```

#### 3. Click
```
POST /click
{
  "selector": "button#submit",
  "index": 0,
  "waitFor": "networkidle"
}
```

#### 4. Type
```
POST /type
{
  "selector": "input[name='search']",
  "text": "Adamas|octa",
  "clear": true,
  "delay": 100,
  "pressEnter": true
}
```

#### 5. Select
```
POST /select
{
  "selector": "select#category",
  "value": "all"
}
```

#### 6. Screenshot
```
GET /screenshot?format=png&fullPage=true
または
POST /screenshot
{
  "format": "png" | "jpeg",
  "fullPage": true | false,
  "selector": "main#content"
}
```

#### 7. Extract
```
POST /extract
{
  "selector": "div.price",
  "attribute": "text" | "href" | "src",
  "extractAll": false,
  "limit": 20,
  "default": "N/A"
}
```

#### 8. Evaluate（JavaScript実行）
```
POST /evaluate
{
  "script": "window.scrollTo(0, document.body.scrollHeight)"
}
```

---

## セレクター戦略

### 優先順位
1. **data-testid**（最優先）
   - 例: `article[data-testid="tweet"]`
   - X、Amazonなど主要サイトで使用
   - ページ更新で変わらない

2. **ID属性**
   - 例: `#productTitle`
   - 比較的安定している

3. **data-* カスタム属性**
   - 例: `[data-hook="review"]`
   - Amazonで多用

4. **クラス名（注意）**
   - 例: `.a-price .a-offscreen`
   - 動的に変わる可能性あり

5. **タグ名（最後の手段）**
   - 例: `a[href*="/status/"]`
   - 曖昧で変更に弱い

---

## エラーハンドリング

### 必須対応
- **タイムアウト:** 指定した時間で要素が見つからない場合の対応
- **要素不在:** セレクターが見つからない場合のデフォルト値返却
- **ページ読み込み失敗:** エラーメッセージを返す

### 期待する挙動
```json
{
  "status": "success" | "error",
  "data": {...},
  "error": {
    "code": "TIMEOUT" | "ELEMENT_NOT_FOUND" | "NAVIGATION_FAILED",
    "message": "Element not found within 15000ms",
    "selector": "article[data-testid='tweet']"
  }
}
```

---

## セッション管理

### 必要な機能
1. **Cookie/LocalStorageの永続化**
   - ログイン状態を維持
   - 再起動後もセッション復元

2. **複数サイトのセッションを独立管理**
   - X, Amazon, Rakutenなどそれぞれ別セッション

3. **ユーザーデータディレクトリの指定**
   - WebView2のUserDataFolderを設定

### 参考実装（Puppeteer）
```javascript
// Cookie保存
const cookies = await page.cookies();
fs.writeFileSync('cookies.json', JSON.stringify(cookies, null, 2));

// 次回起動時
const cookies = JSON.parse(fs.readFileSync('cookies.json'));
await page.setCookie(...cookies);
```

---

## パフォーマンス要件

### 期待するパフォーマンス
- **ページ読み込み:** < 5秒（networkidle）
- **要素取得:** < 1秒
- **スクリーンショット:** < 2秒

### 最適化のポイント
1. ヘッドレスモード（headless）
2. 不要なリソースのブロック（画像、CSSフォント）
3. ユーザーエージェントの適切設定
4. キャッシュの有効化

---

## テスト実行フェーズ

### Phase 1: 基本操作（High優先度）
1. UC-01: Xエゴサーチ
2. UC-02: Google Shopping価格調査
3. UC-10: ログイン認証

### Phase 2: ECサイト巡回（Medium優先度）
4. UC-03: Amazon商品確認
5. UC-04: Rakuten RMS注文管理
6. UC-05: Yahooショッピング商品確認

### Phase 3: 高度な操作（Low優先度）
7. UC-06: 楽天ブックスレビュークロール
8. UC-07: ECサイト広告確認
9. UC-08: Adamas公式サイト巡回
10. UC-09: ターゲットサイトクロール

---

## 提出物

### 期待する報告形式
```json
{
  "test_id": "UC-01",
  "status": "success" | "failure" | "partial",
  "execution_time_ms": 1234,
  "errors": [
    {
      "step": 2,
      "message": "Selector not found",
      "selector": "button#submit"
    }
  ],
  "screenshots": ["path/to/screenshot.png"],
  "extracted_data": {...}
}
```

### 成功基準
- [ ] 全ての基本操作APIが機能する（navigate, click, type, select, screenshot, extract）
- [ ] ログイン認証が成功する（少なくとも1つのサイト）
- [ ] ページ遷移が正常に動作する
- [ ] スクリーンショットが取得できる
- [ ] DOM要素の抽出ができる

---

## 重要な決定事項

### ポート番号
- **Axum APIサーバー:** 9400（現在は応答なし）

### 開発環境
- **現在:** WSL側で開発 → Windows側に手動コピー → ビルド・実行
- **提案:** Windows側の `C:\Users\mhit\Documents\GitHub` に移動して統合

### ビルド・実行手順（現在）
```bash
# 1. 既存プロセス終了
taskkill /F /IM webview-bridge-rust.exe /T

# 2. WSL→Windowsへ同期（手動コピー）
cp -r /home/mhit/webview-bridge/* /mnt/c/Users/mhit/AppData/Local/Temp/webview-bridge-build/

# 3. ビルド＆実行（STAモード必須）
cd C:\Users\mhit\AppData\Local\Temp\webview-bridge-build
CARGO_INCREMENTAL=0 cargo.exe run
```

---

## 開発担当AIへの質問

1. **Axumサーバー:** ポート9400に応答しない原因は？
2. **真っ白画面:** WebView2の描画が機能していない原因は？
3. **リサイズ追従:** 実装予定はあるか？
4. **開発環境:** Windows側に統合すべきか？

---

## 連絡先

### プロジェクトオーナー
- **Hitoshi (松下 仁志)**
- **Email:** matsushita@cis1212.co.jp

### 現行スクリプトの場所
- **スクリプトディレクトリ:** `/home/mhit/clawd/scripts/`
- **参考スクリプト:**
  - egosearch-puppeteer.mjs
  - scrape-amazon-reviews.mjs
  - x-auto-login.mjs
  - browser-pool.mjs

---

**作成日:** 2026-02-05
**作成者:** Sam (OpenClaw Assistant)
**用途:** WebView Bridge開発担当AIへ渡すためのサマリー
