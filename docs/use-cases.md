# WebView Bridge ユースケース

## 1. ECサイトスクレイピング

### 1.1 商品情報収集

**シナリオ**: Amazon、楽天、Yahoo等から商品情報を定期収集

```
現状の問題:
- Puppeteerでbot検出されやすい
- Cloudflare保護でブロック
- ログインセッション維持が困難

WebView Bridgeでの解決:
- 通常ブラウザと同じ挙動でbot検出回避
- プロファイルでログイン状態を永続化
- 安定した定期実行
```

**APIフロー例**:
```http
# 1. セッション作成（ECサイト用プロファイル）
POST /api/sessions
{"profile": "ecommerce", "headless": true}
→ {"sessionId": "sess_001"}

# 2. 商品ページにナビゲート
POST /api/sessions/sess_001/navigate
{"url": "https://amazon.co.jp/dp/XXXXXXXX", "waitUntil": "networkidle"}

# 3. 商品情報を抽出
POST /api/sessions/sess_001/query
{
  "queries": [
    {"name": "title", "selector": "#productTitle", "type": "text"},
    {"name": "price", "selector": ".a-price-whole", "type": "text"},
    {"name": "rating", "selector": ".a-icon-star-small", "type": "attribute", "attr": "class"},
    {"name": "reviews", "selector": "#acrCustomerReviewText", "type": "text"},
    {"name": "availability", "selector": "#availability span", "type": "text"}
  ]
}
→ {"title": "商品名", "price": "1,980", ...}

# 4. セッション終了
DELETE /api/sessions/sess_001
```

### 1.2 価格監視

**シナリオ**: 競合商品の価格を定期監視

```
要件:
- 1日数回の定期チェック
- 複数サイトの同一商品比較
- 価格変動時にアラート

実装:
- cronジョブから定期呼び出し
- 複数セッション並列実行で高速化
- 結果をDBに保存、差分検出
```

### 1.3 レビュー収集

**シナリオ**: 自社商品のレビューを収集・分析

```
現状の問題:
- 無限スクロールの処理が複雑
- レート制限による中断
- ログイン必要なサイトがある

WebView Bridgeでの解決:
- scroll + waitForSelector で無限スクロール対応
- 適切なインターバルでレート制限回避
- プロファイルでログイン状態維持
```

---

## 2. SNSモニタリング

### 2.1 X (Twitter) エゴサーチ

**シナリオ**: 自社ブランド名の言及を監視

```
現状の問題:
- X APIの制限が厳しい（有料化）
- ログイン必須
- 頻繁な仕様変更

WebView Bridgeでの解決:
- ログイン状態をプロファイルで維持
- 検索結果ページをスクレイピング
- API制限なし
```

**APIフロー例**:
```http
# 1. X用プロファイルでセッション作成
POST /api/sessions
{"profile": "twitter", "headless": true}

# 2. 検索ページにナビゲート
POST /api/sessions/{id}/navigate
{"url": "https://x.com/search?q=Adamas%20Octa&f=live"}

# 3. ツイート一覧を取得
POST /api/sessions/{id}/evaluate
{
  "script": `
    Array.from(document.querySelectorAll('[data-testid="tweet"]')).map(t => ({
      text: t.querySelector('[data-testid="tweetText"]')?.textContent,
      author: t.querySelector('[data-testid="User-Name"]')?.textContent,
      time: t.querySelector('time')?.getAttribute('datetime')
    }))
  `
}
```

### 2.2 Instagram監視

**シナリオ**: ハッシュタグ投稿の監視

```
要件:
- 特定ハッシュタグの新規投稿検出
- 投稿画像の保存
- エンゲージメント数の取得
```

### 2.3 YouTube Analytics

**シナリオ**: 競合チャンネルの動画パフォーマンス分析

```
要件:
- チャンネルの動画一覧取得
- 再生数、高評価数、コメント数
- トレンド分析
```

---

## 3. ログイン必須サービス

### 3.1 管理画面操作

**シナリオ**: 各種管理画面からデータ取得

```
対象サービス例:
- Google Analytics
- Google Search Console
- 楽天RMS
- Yahoo!ストアクリエイター

WebView Bridgeのメリット:
- 2FA対応（初回のみ手動認証、以降はセッション維持）
- 複雑なSPA対応
- APIがないサービスでもデータ取得可能
```

### 3.2 銀行・金融サービス

**シナリオ**: 入出金明細の自動取得

```
注意事項:
- セキュリティ上、特に慎重な取り扱い必要
- 認証情報の暗号化保存
- 操作ログの詳細記録
```

### 3.3 社内システム連携

**シナリオ**: レガシー社内システムからのデータ抽出

```
よくあるケース:
- APIのない古いWebシステム
- 特定ブラウザでしか動かない
- 手作業が必要だったレポート取得
```

---

## 4. Web自動化

### 4.1 フォーム自動入力

**シナリオ**: 定型的な申請処理の自動化

```http
POST /api/sessions/{id}/action
{
  "actions": [
    {"type": "type", "selector": "#company", "text": "株式会社シイ・アイ・エス"},
    {"type": "type", "selector": "#name", "text": "松下仁志"},
    {"type": "type", "selector": "#email", "text": "matsushita@cis1212.co.jp"},
    {"type": "select", "selector": "#prefecture", "value": "富山県"},
    {"type": "click", "selector": "#agree"},
    {"type": "click", "selector": "#submit"},
    {"type": "waitForSelector", "selector": ".success-message"}
  ]
}
```

### 4.2 予約システム

**シナリオ**: 人気施設の予約を自動で試行

```
要件:
- 特定時刻に自動アクセス
- 空き状況のリアルタイム確認
- 予約完了まで自動操作
```

### 4.3 データエクスポート

**シナリオ**: CSVエクスポート機能がないサービスからデータ取得

```
アプローチ:
1. 一覧ページをスクレイピング
2. ページネーション対応
3. 全データをJSON/CSVで保存
```

---

## 5. コンテンツ収集

### 5.1 ニュース記事収集

**シナリオ**: 業界ニュースの自動収集

```
対象:
- 日経新聞（有料会員ログイン）
- 業界専門メディア
- プレスリリース

処理:
1. RSSで新着検知
2. WebView Bridgeで本文取得
3. LLMで要約・分類
4. 日報に反映
```

### 5.2 求人情報収集

**シナリオ**: 競合の採用動向モニタリング

```
対象:
- Indeed
- リクナビ
- 企業採用ページ

取得情報:
- 募集職種
- 給与レンジ
- 必須スキル
```

### 5.3 法務情報収集

**シナリオ**: 登記情報、特許情報の取得

```
対象:
- 登記情報提供サービス
- J-PlatPat（特許検索）
- EDINET（有価証券報告書）
```

---

## 6. モニタリング・監視

### 6.1 サイト死活監視

**シナリオ**: 自社サイトの表示確認

```
従来のpingやHTTPチェックとの違い:
- JavaScript実行後の表示確認
- 実際のレンダリング結果をスクリーンショット
- 特定要素の表示確認
```

**APIフロー例**:
```http
# 1. ナビゲート
POST /api/sessions/{id}/navigate
{"url": "https://shop.adamas-octa.com", "timeout": 10000}

# 2. 重要要素の存在確認
POST /api/sessions/{id}/query
{
  "checks": [
    {"selector": "header", "exists": true},
    {"selector": ".product-grid", "minCount": 1},
    {"selector": ".error-page", "exists": false}
  ]
}

# 3. スクリーンショット保存
POST /api/sessions/{id}/screenshot
{"fullPage": true, "format": "png"}
```

### 6.2 パフォーマンス監視

**シナリオ**: ページ読み込み速度の継続監視

```
取得メトリクス:
- Time to First Byte (TTFB)
- First Contentful Paint (FCP)
- Largest Contentful Paint (LCP)
- Total Load Time

実装:
WebView2のPerformance APIを利用
```

### 6.3 SEO監視

**シナリオ**: 検索順位の定期チェック

```
対象:
- Google検索結果
- Yahoo検索結果

取得:
- 特定キーワードでの順位
- 表示されるスニペット
- 競合の順位
```

---

## 7. OpenClaw統合シナリオ

### 7.1 Agentからの直接利用

**シナリオ**: Claudeが自律的にWeb情報を取得

```
現状:
- browser toolが不安定
- Cloudflareでブロックされることがある

WebView Bridge導入後:
- 安定したWeb操作
- ログイン状態の維持
- 複雑なSPAも操作可能
```

**OpenClaw設定例**:
```yaml
tools:
  browser:
    provider: webview-bridge
    endpoint: http://localhost:9400
    profiles:
      default: 一般的なブラウジング
      authenticated: ログイン済みサービス
```

### 7.2 定期タスクでの利用

**シナリオ**: cronジョブからのスクレイピング

```yaml
# OpenClaw cron設定
jobs:
  - name: daily-egosearch
    schedule: "0 8 * * *"
    payload:
      kind: agentTurn
      message: |
        WebView Bridgeを使って以下を実行:
        1. X検索で「Adamas Octa」を検索
        2. 新しい言及があれば報告
```

### 7.3 複合タスク

**シナリオ**: 複数サイトを横断する情報収集

```
例: 商品価格比較
1. Amazon.co.jpで価格取得
2. 楽天市場で価格取得
3. Yahoo!ショッピングで価格取得
4. 結果を比較・レポート

並列実行で高速化
```

---

## 8. 開発・テスト利用

### 8.1 E2Eテスト

**シナリオ**: Webアプリケーションの自動テスト

```
メリット:
- 本番環境と同じブラウザエンジン
- CIパイプラインでの利用
- スクリーンショットによるビジュアルリグレッション
```

### 8.2 スクリーンショット生成

**シナリオ**: ドキュメント用のスクリーンショット自動生成

```
用途:
- マニュアル作成
- リリースノート
- プレゼン資料
```

### 8.3 PDFレンダリング

**シナリオ**: HTMLからPDF生成

```
WebView2のPrint to PDF機能を活用:
- レポートのPDF化
- 請求書生成
- 証跡保存
```

---

## 9. セキュリティ考慮事項

### 9.1 認証情報の取り扱い

```
⚠️ 注意が必要なケース:
- 銀行サイトのスクレイピング
- 個人情報を含むページ
- 社内システムへのアクセス

対策:
- プロファイルごとの権限分離
- 操作ログの詳細記録
- 認証情報の暗号化保存
```

### 9.2 利用規約の確認

```
各サービスの利用規約を確認:
- スクレイピング禁止条項
- 自動アクセスの制限
- データ利用の制限

グレーゾーンの場合:
- アクセス頻度を抑える
- User-Agentを偽装しない
- robots.txtを確認
```

### 9.3 データ保護

```
取得したデータの取り扱い:
- 個人情報の適切な管理
- 不要データの速やかな削除
- アクセスログの保持期間
```

---

## 10. ユースケース優先度

### 高優先度（すぐに使いたい）
1. ECサイトスクレイピング（価格・レビュー）
2. X/SNSエゴサーチ
3. OpenClaw browser tool代替

### 中優先度（あると便利）
4. 管理画面データ取得
5. サイト監視
6. フォーム自動化

### 低優先度（将来的に）
7. PDF生成
8. E2Eテスト
9. 複雑な業務自動化
