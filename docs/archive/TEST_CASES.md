# WebView Bridge Test Cases - 実運用ユースケース

## 概要
Sam（OpenClaw Assistant）が通常業務で使用するWebアクセス操作をテストケースとして定義。
開発担当AIへ渡して、実装・検証に使用する。

---

## 基本操作API（想定）

### Navigation
```
POST /navigate
{
  "url": "https://example.com",
  "waitFor": "networkidle" | "selector:xxx" | "timeout:5000"
}
```

### Click
```
POST /click
{
  "selector": "button#submit",
  "waitFor": "networkidle"
}
```

### Type
```
POST /type
{
  "selector": "input[name=search]",
  "text": "Adamas|octa",
  "clear": true,
  "pressEnter": true
}
```

### Select
```
POST /select
{
  "selector": "select#category",
  "value": "all" | "option text"
}
```

### Screenshot
```
GET /screenshot
{
  "format": "png" | "jpeg",
  "fullPage": true | false,
  "selector": "main#content" (optional)
}
```

### Extract
```
POST /extract
{
  "selector": "div.price",
  "attribute": "text" | "href" | "src" | "data-xxx"
}
```

### Execute JavaScript
```
POST /evaluate
{
  "script": "document.querySelector('xxx').scrollIntoView()"
}
```

---

## ユースケース別テストケース

### UC-01: X（Twitter）エゴサーチ

#### 目的
Adamas|octaのブランド言及を検索し、ユーザーからの反応を収集する。

#### 前提条件
- Xにログイン済み（Cookie保持）

#### 操作手順
```json
{
  "test_id": "UC-01",
  "name": "Xエゴサーチ",
  "steps": [
    {
      "action": "navigate",
      "url": "https://x.com/search?q=Adamas%7Cocta&src=typed_query&f=live"
    },
    {
      "action": "waitForSelector",
      "selector": "article[data-testid='tweet']"
    },
    {
      "action": "screenshot",
      "format": "png",
      "fullPage": true
    },
    {
      "action": "extract",
      "selector": "article[data-testid='tweet']",
      "extract": [
        "text",
        "time",
        "author",
        "likes"
      ],
      "count": 10
    }
  ],
  "expected_result": [
    "最新のツイートが10件抽出される",
    "スクリーンショットが取得できる",
    "各ツイートのテキスト・日時・著者・いいね数が取得できる"
  ]
}
```

#### テストデータ
- 検索キーワード: `Adamas|octa`
- 期待件数: 少なくとも1件以上のツイート

---

### UC-02: Google Shopping価格調査

#### 目的
Adamas|octaの競合商品の価格を調査し、市場価格を把握する。

#### 前提条件
- Googleアカウントでログイン（不要だが、結果が変わる場合あり）

#### 操作手順
```json
{
  "test_id": "UC-02",
  "name": "Google Shopping価格調査",
  "steps": [
    {
      "action": "navigate",
      "url": "https://shopping.google.com/"
    },
    {
      "action": "type",
      "selector": "input[name='q']",
      "text": "Adamas|octa コーティング",
      "clear": true,
      "pressEnter": true
    },
    {
      "action": "waitForSelector",
      "selector": "div.sh-osd__cont"
    },
    {
      "action": "screenshot",
      "format": "jpeg",
      "fullPage": false
    },
    {
      "action": "extract",
      "selector": "div.sh-pr__product-results",
      "extract": [
        "product_name",
        "price",
        "store",
        "rating"
      ],
      "count": 20
    }
  ],
  "expected_result": [
    "検索結果が表示される",
    "商品情報（名前・価格・店舗・評価）が抽出できる",
    "スクリーンショットが取得できる"
  ]
}
```

#### テストデータ
- 検索キーワード: `Adamas|octa コーティング`

---

### UC-03: Amazon商品確認

#### 目的
Adamas|octa商品のAmazonページを確認し、価格・在庫・レビュー状況を把握する。

#### 前提条件
- Amazonアカウントでログイン（推奨）

#### 操作手順
```json
{
  "test_id": "UC-03",
  "name": "Amazon商品確認",
  "steps": [
    {
      "action": "navigate",
      "url": "https://www.amazon.co.jp/s?k=Adamas+%7C+octa"
    },
    {
      "action": "waitForSelector",
      "selector": "[data-component-type='s-search-result']"
    },
    {
      "action": "click",
      "selector": "[data-component-type='s-search-result'] a",
      "index": 0
    },
    {
      "action": "waitForSelector",
      "selector": "#productTitle, #centerCol"
    },
    {
      "action": "screenshot",
      "format": "png",
      "fullPage": true
    },
    {
      "action": "extract",
      "selector": "#centerCol",
      "extract": [
        "title: #productTitle",
        "price: .a-price .a-offscreen",
        "availability: #availability span",
        "rating: [data-hook='average-star-rating']",
        "review_count: [data-hook='total-review-count']"
      ]
    }
  ],
  "expected_result": [
    "商品詳細ページが表示される",
    "商品タイトル・価格・在庫状況・評価・レビュー数が抽出できる",
    "スクリーンショットが取得できる"
  ]
}
```

---

### UC-04: Rakuten RMS（楽天市場）注文管理

#### 目的
楽天市場での注文状況を確認し、受注数を把握する。

#### 前提条件
- RMSアカウントでログイン済み

#### 操作手順
```json
{
  "test_id": "UC-04",
  "name": "Rakuten RMS注文管理",
  "steps": [
    {
      "action": "navigate",
      "url": "https://rms.rakuten.co.jp/rms/mall/order/"
    },
    {
      "action": "waitForSelector",
      "selector": "table.order-list, .orderData"
    },
    {
      "action": "screenshot",
      "format": "jpeg",
      "fullPage": false
    },
    {
      "action": "extract",
      "selector": "table.order-list tbody tr",
      "extract": [
        "order_id",
        "order_date",
        "customer_name",
        "total_amount",
        "status"
      ],
      "count": 50
    }
  ],
  "expected_result": [
    "注文一覧が表示される",
    "注文ID・日時・顧客名・合計金額・ステータスが抽出できる",
    "スクリーンショットが取得できる"
  ]
}
```

#### 注意点
- RMSはJavaScriptが多用されているため、`waitFor: networkidle` が重要
- 認証セッションの管理が必要

---

### UC-05: Yahooショッピング商品確認

#### 目的
Adamas|octaのYahooショッピング店舗を確認し、商品表示状況を把握する。

#### 前提条件
- Yahoo店舗管理アカウントでログイン（店舗オーナーの場合）

#### 操作手順
```json
{
  "test_id": "UC-05",
  "name": "Yahooショッピング商品確認",
  "steps": [
    {
      "action": "navigate",
      "url": "https://store.shopping.yahoo.co.jp/adamasocta/"
    },
    {
      "action": "waitForSelector",
      "selector": "elItem"
    },
    {
      "action": "screenshot",
      "format": "png",
      "fullPage": true
    },
    {
      "action": "extract",
      "selector": "elItem",
      "extract": [
        "product_name",
        "price",
        "image_url",
        "product_link"
      ],
      "count": 20
    }
  ],
  "expected_result": [
    "商品一覧が表示される",
    "商品情報（名前・価格・画像・リンク）が抽出できる",
    "スクリーンショットが取得できる"
  ]
}
```

---

### UC-06: 楽天ブックスレビュークロール

#### 目的
Adamas|octa関連商品の楽天ブックスレビューをクロールし、顧客の声を収集する。

#### 前提条件
- なし

#### 操作手順
```json
{
  "test_id": "UC-06",
  "name": "楽天ブックスレビュークロール",
  "steps": [
    {
      "action": "navigate",
      "url": "https://books.rakuten.co.jp/search?g=001&qt=Adamas|octa"
    },
    {
      "action": "waitForSelector",
      "selector": ".rb-item__title a"
    },
    {
      "action": "click",
      "selector": ".rb-item__title a",
      "index": 0
    },
    {
      "action": "waitForSelector",
      "selector": ".revRvwerList, .reviews"
    },
    {
      "action": "screenshot",
      "format": "png",
      "fullPage": true
    },
    {
      "action": "extract",
      "selector": ".revRvwerList .reviewItem, .reviews .review",
      "extract": [
        "rating",
        "title",
        "content",
        "author",
        "date"
      ],
      "count": 10
    }
  ],
  "expected_result": [
    "商品詳細ページが表示される",
    "レビュー情報（評価・タイトル・内容・著者・日時）が抽出できる",
    "スクリーンショットが取得できる"
  ]
}
```

#### テストデータ
- 検索キーワード: `Adamas|octa`

---

### UC-07: ECサイト広告確認

#### 目的
主要ECサイト（Amazon・楽天・Yahoo）でAdamas|octaの広告表示状況を確認する。

#### 前提条件
- 各サイトのアカウントでログイン（推奨）

#### 操作手順（Amazon）
```json
{
  "test_id": "UC-07-Amazon",
  "name": "Amazon広告確認",
  "steps": [
    {
      "action": "navigate",
      "url": "https://www.amazon.co.jp/"
    },
    {
      "action": "type",
      "selector": "#twotabsearchtextbox",
      "text": "Adamas|octa",
      "clear": true,
      "pressEnter": true
    },
    {
      "action": "waitForSelector",
      "selector": "[data-component-type='s-search-result']"
    },
    {
      "action": "screenshot",
      "format": "jpeg",
      "fullPage": false
    },
    {
      "action": "evaluate",
      "script": "document.querySelectorAll('[data-component-type='s-search-result'] .sponsored-label').length"
    },
    {
      "action": "extract",
      "selector": "[data-component-type='s-search-result']",
      "extract": [
        "is_sponsored: .sponsored-label",
        "product_name",
        "price",
        "ad_position"
      ],
      "count": 20
    }
  ],
  "expected_result": [
    "検索結果が表示される",
    "広告ラベルの有無が判定できる",
    "広告の位置情報が取得できる"
  ]
}
```

#### 同様の操作を楽天・Yahooでも実行

---

### UC-08: Adamas公式サイト巡回

#### 目的
Adamas|octa公式サイトの更新情報を確認し、キャンペーン等の情報を収集する。

#### 前提条件
- なし

#### 操作手順
```json
{
  "test_id": "UC-08",
  "name": "Adamas公式サイト巡回",
  "steps": [
    {
      "action": "navigate",
      "url": "https://shop.adamas-octa.com/"
    },
    {
      "action": "waitForSelector",
      "selector": "main, #main"
    },
    {
      "action": "screenshot",
      "format": "png",
      "fullPage": true
    },
    {
      "action": "extract",
      "selector": "main, #main",
      "extract": [
        "campaigns: .campaign, .banner",
        "news: .news, .announcement",
        "popular_products: .popular, .ranking"
      ]
    }
  ],
  "expected_result": [
    "サイトが正常に表示される",
    "キャンペーン・お知らせ・人気商品情報が抽出できる",
    "スクリーンショットが取得できる"
  ]
}
```

---

### UC-09: ターゲットサイトクロール

#### 目的
特定のURLからページ内のリンクを抽出し、関連ページを巡回する。

#### 前提条件
- なし

#### 操作手順
```json
{
  "test_id": "UC-09",
  "name": "ターゲットサイトクロール",
  "steps": [
    {
      "action": "navigate",
      "url": "https://example.com/target-page"
    },
    {
      "action": "waitForSelector",
      "selector": "a[href]"
    },
    {
      "action": "extract",
      "selector": "a[href]",
      "attribute": "href",
      "count": 50
    },
    {
      "action": "click",
      "selector": "a[href]",
      "index": 0,
      "waitFor": "networkidle"
    },
    {
      "action": "screenshot",
      "format": "png",
      "fullPage": false
    }
  ],
  "expected_result": [
    "ページ内のリンクが抽出できる",
    "リンク先のページに遷移できる",
    "スクリーンショットが取得できる"
  ]
}
```

---

### UC-10: ログイン認証

#### 目的
各サイトへのログイン機能を確認し、セッション管理を検証する。

#### 前提条件
- ログイン情報（ID/パスワード）があること

#### 操作手順（X）
```json
{
  "test_id": "UC-10-X",
  "name": "Xログイン",
  "steps": [
    {
      "action": "navigate",
      "url": "https://x.com/i/flow/login"
    },
    {
      "action": "waitForSelector",
      "selector": "input[name='text'], input[name='username']"
    },
    {
      "action": "type",
      "selector": "input[name='text'], input[name='username']",
      "text": "{USER_ID}",
      "clear": true,
      "pressEnter": true
    },
    {
      "action": "waitForSelector",
      "selector": "input[name='password']"
    },
    {
      "action": "type",
      "selector": "input[name='password']",
      "text": "{PASSWORD}",
      "clear": true,
      "pressEnter": true
    },
    {
      "action": "waitForSelector",
      "selector": "[data-testid='SideNav_AccountSwitcher_Button']",
      "timeout": 10000
    },
    {
      "action": "screenshot",
      "format": "png",
      "fullPage": false
    },
    {
      "action": "evaluate",
      "script": "document.cookie"
    }
  ],
  "expected_result": [
    "ログインが成功する",
    "セッションクッキーが保存される",
    "スクリーンショットが取得できる"
  ]
}
```

#### 注意点
- パスワードは環境変数や設定ファイルから読み込むこと
- セッション管理（Cookie保存・復元）が必要
- 2要素認証があるサイトは別途対応が必要

---

## 共通要件

### セレクター戦略
- **優先順位**: data-testid > id > class > タグ
- **安定性**: ページ更新で変わらないセレクターを使用
- **フォールバック**: 複数のセレクターパターンを用意

### エラーハンドリング
- セレクターが見つからない場合: タイムアウト設定と再試行
- ページ読み込み失敗: エラーメッセージを返す
- ネットワークエラー: 再試行ロジック

### セッション管理
- Cookie/LocalStorageの永続化
- 複数サイトのセッションを独立して管理
- セッション有効期限のチェック

### レート制限
- 連続アクセスの間隔を設定
- サイトごとの制限を尊重
- 429エラー時の待機処理

---

## テスト実行順序

### Phase 1: 基本操作
1. UC-10: ログイン認証（各サイト）
2. UC-01: Xエゴサーチ
3. UC-02: Google Shopping価格調査

### Phase 2: ECサイト巡回
4. UC-03: Amazon商品確認
5. UC-04: Rakuten RMS注文管理
6. UC-05: Yahooショッピング商品確認

### Phase 3: 高度な操作
7. UC-06: 楽天ブックスレビュークロール
8. UC-07: ECサイト広告確認
9. UC-08: Adamas公式サイト巡回
10. UC-09: ターゲットサイトクロール

---

## 成功基準

### 必須
- [ ] 全ての基本操作APIが機能する（navigate, click, type, select, screenshot, extract）
- [ ] ログイン認証が成功する（少なくとも1つのサイト）
- [ ] ページ遷移が正常に動作する
- [ ] スクリーンショットが取得できる
- [ ] DOM要素の抽出ができる

### 期待
- [ ] 複数サイトのセッション管理ができる
- [ ] JavaScript実行ができる
- [ ] レート制限に対処できる
- [ ] エラーハンドリングが機能する

---

## 開発担当AIへの指示

### 優先度
1. **High**: UC-01〜UC-04（基本的なWebアクセス）
2. **Medium**: UC-05〜UC-08（ECサイト巡回）
3. **Low**: UC-09〜UC-10（高度な操作）

### 提出物
1. 各テストケースの実行結果（成功/失敗）
2. 期待結果との差分
3. バグ・課題一覧
4. パフォーマンス測定（レスポンスタイム）

### 報告形式
```json
{
  "test_id": "UC-XX",
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

---

**作成日:** 2026-02-05
**作成者:** Sam (OpenClaw Assistant)
**用途:** WebView Bridge（Rust版）の実運用テスト
