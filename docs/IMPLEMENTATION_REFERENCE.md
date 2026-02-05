# 現行実装参考資料

## 概要
Samが現在運用しているWebスクレイピングスクリプトの実装パターンを整理。
WebView Bridgeの実装参考にするための資料。

---

## 参考スクリプト一覧

### 1. egosearch-puppeteer.mjs - Xエゴサーチ

**場所:** `/home/mhit/clawd/scripts/egosearch-puppeteer.mjs`

**目的:** Adamas|octa関連のX（Twitter）での言及を収集

**実装パターン:**
```javascript
import puppeteer from 'puppeteer-core';

// Chrome Profile使用（ログイン済み）
const PROFILE_PATH = process.env.CHROME_PROFILE || '/home/mhit/.config/chrome-automation';
const CDP_PORT = process.env.CDP_PORT || 9222;

// 起動
const browser = await puppeteer.launch({
  executablePath: '/usr/bin/google-chrome',
  headless: process.env.HEADLESS === 'true',
  args: [
    `--user-data-dir=${PROFILE_PATH}`,
    `--remote-debugging-port=${CDP_PORT}`,
    '--no-first-run',
    '--disable-blink-features=AutomationControlled'
  ]
});

// ページ操作
const page = await browser.newPage();
await page.setUserAgent('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36...');

// ナビゲーション
await page.goto(url, { waitUntil: 'networkidle2', timeout: 30000 });

// 要素待機
await page.waitForSelector('article[data-testid="tweet"]', { timeout: 15000 });

// データ抽出
const tweets = await page.evaluate(() => {
  const elements = document.querySelectorAll('article[data-testid="tweet"]');
  return Array.from(elements).map(el => ({
    text: el.querySelector('[data-testid="tweetText"]')?.innerText,
    author: el.querySelector('[data-testid="User-Name"] span')?.innerText,
    time: el.querySelector('time')?.getAttribute('datetime'),
    url: el.querySelector('a[href*="/status/"]')?.href
  }));
});

// スクリーンショット
await page.screenshot({ path: 'screenshot.png', fullPage: true });
```

**ポイント:**
- ログイン済みプロファイルを使用（Cookie維持）
- `networkidle2` でページ読み込み完了を待機
- `page.evaluate()` でDOM操作
- `data-testid` 属性を優先的に使用

---

### 2. scrape-amazon-reviews.mjs - Amazonレビュースクレイピング

**場所:** `/home/mhit/clawd/scripts/scrape-amazon-reviews.mjs`

**目的:** Amazon商品ページからレビューを抽出

**実装パターン:**
```javascript
// URL構築
const url = `https://www.amazon.co.jp/product-reviews/${asin}?filterByStar=all_stars`;

// ページ操作
await page.goto(url, { waitUntil: 'networkidle2', timeout: 30000 });

// 要素待機
await page.waitForSelector('[data-hook="review"]', { timeout: 15000 }).catch(() => null);

// 複雑なデータ抽出
const reviews = await page.evaluate(() => {
  const reviewElements = document.querySelectorAll('[data-hook="review"]');
  const results = [];
  
  reviewElements.forEach(el => {
    const ratingEl = el.querySelector('[data-hook="review-star-rating"]');
    const ratingMatch = ratingEl?.className?.match(/a-star-(\d)/);
    const rating = ratingMatch ? parseInt(ratingMatch[1]) : null;
    
    const titleEl = el.querySelector('[data-hook="review-title"] span');
    const title = titleEl?.innerText?.trim() || '';
    
    const bodyEl = el.querySelector('[data-hook="review-body"] span');
    const body = bodyEl?.innerText?.trim() || '';
    
    // ... その他フィールド
    
    results.push({ rating, title, body, ... });
  });
  
  return results;
});
```

**ポイント:**
- 正規表現でクラス名から数値を抽出（評価など）
- `innerText?.trim()` で空白除去
- `.catch(() => null)` でタイムアウト時に例外を無視
- ネストされたクエリセレクター

---

### 3. x-auto-login.mjs - X自動ログイン

**場所:** `/home/mhit/clawd/scripts/x-auto-login.mjs`

**目的:** Xへの自動ログイン

**実装パターン:**
```javascript
// 認証情報読み込み
const auth = JSON.parse(fs.readFileSync('~/.credentials/x-login.json', 'utf8'));

// ログインフロー
await page.goto('https://x.com/i/flow/login');

// ユーザー名入力
await page.waitForSelector('input[name="text"], input[name="username"]');
await page.type('input[name="text"]', auth.username, { delay: 100 });
await page.keyboard.press('Enter');

// パスワード入力
await page.waitForSelector('input[name="password"]');
await page.type('input[name="password"]', auth.password, { delay: 100 });
await page.keyboard.press('Enter');

// 2要素認証対応（必要な場合）
await page.waitForSelector('input[name="text"], input[data-testid="ocfEnterTextTextInput"]');
// ...

// ログイン完了確認
await page.waitForSelector('[data-testid="SideNav_AccountSwitcher_Button"]', { timeout: 10000 });

// セッション保存（Cookie）
const cookies = await page.cookies();
fs.writeFileSync('~/.config/x-cookies.json', JSON.stringify(cookies, null, 2));
```

**ポイント:**
- `{ delay: 100 }` で人間らしい入力間隔
- `keyboard.press('Enter')` でフォーム送信
- セッション情報をファイルに保存
- タイムアウト設定で無限待機を回避

---

### 4. browser-pool.mjs - ブラウザープール管理

**場所:** `/home/mhit/clawd/scripts/browser-pool.mjs`

**目的:** 複数のブラウザインスタンスをプール管理

**実装パターン:**
```javascript
// インスタンス取得
async function acquireBrowser(purpose = 'default') {
  const port = findAvailablePort(9300, 9399);
  const instance = {
    id: generateId(),
    port,
    purpose,
    pid: null,
    status: 'acquired',
    createdAt: Date.now()
  };
  
  // Chrome起動
  const args = [
    '--headless',
    '--no-sandbox',
    `--remote-debugging-port=${port}`,
    `--user-data-dir=${getProfilePath(instance.id)}`
  ];
  
  instance.pid = spawn('/usr/bin/google-chrome', args).pid;
  pool.set(instance.id, instance);
  
  return instance;
}

// インスタンス解放
async function releaseBrowser(instanceId) {
  const instance = pool.get(instanceId);
  if (instance && instance.pid) {
    process.kill(instance.pid);
    pool.delete(instanceId);
  }
}

// ステータス確認
function getPoolStatus() {
  return Array.from(pool.values()).map(instance => ({
    id: instance.id,
    port: instance.port,
    purpose: instance.purpose,
    status: instance.status,
    age: Date.now() - instance.createdAt
  }));
}
```

**ポイント:**
- ポート範囲で競合回避（9300-9399）
- PID管理で確実なプロセス終了
- 目的（purpose）ごとにインスタンス管理
- ユーザーデータディレクトリを分離

---

## WebView Bridgeへの適用

### 必要なAPI

#### 1. Navigate
```javascript
// 現行（Puppeteer）
await page.goto(url, { waitUntil: 'networkidle2', timeout: 30000 });

// WebView Bridge（想定）
POST /navigate
{
  "url": "https://example.com",
  "waitFor": "networkidle",
  "timeout": 30000
}
```

#### 2. Wait for Selector
```javascript
// 現行（Puppeteer）
await page.waitForSelector('article[data-testid="tweet"]', { timeout: 15000 });

// WebView Bridge（想定）
POST /wait
{
  "selector": "article[data-testid='tweet']",
  "timeout": 15000
}
```

#### 3. Click
```javascript
// 現行（Puppeteer）
await page.click('button.submit');

// WebView Bridge（想定）
POST /click
{
  "selector": "button.submit"
}
```

#### 4. Type
```javascript
// 現行（Puppeteer）
await page.type('input[name="search"]', 'Adamas|octa', { delay: 100 });
await page.keyboard.press('Enter');

// WebView Bridge（想定）
POST /type
{
  "selector": "input[name='search']",
  "text": "Adamas|octa",
  "delay": 100,
  "pressEnter": true
}
```

#### 5. Extract
```javascript
// 現行（Puppeteer）
const data = await page.evaluate(() => {
  const el = document.querySelector('div.price');
  return el?.innerText;
});

// WebView Bridge（想定）
POST /extract
{
  "selector": "div.price",
  "attribute": "text"
}
```

#### 6. Screenshot
```javascript
// 現行（Puppeteer）
await page.screenshot({ path: 'screenshot.png', fullPage: true });

// WebView Bridge（想定）
GET /screenshot
{
  "format": "png",
  "fullPage": true
}
```

#### 7. Evaluate（JavaScript実行）
```javascript
// 現行（Puppeteer）
const result = await page.evaluate(() => {
  return window.scrollTo(0, document.body.scrollHeight);
});

// WebView Bridge（想定）
POST /evaluate
{
  "script": "window.scrollTo(0, document.body.scrollHeight)"
}
```

---

## セレクター戦略

### 優先順位（Puppeteer実装での使用傾向）

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
   - 複数のクラスを指定することで安定性向上

5. **タグ名（最後の手段）**
   - 例: `a[href*="/status/"]`
   - 曖昧で変更に弱い

---

## エラーハンドリングパターン

### タイムアウト対応
```javascript
// Puppeteer
await page.waitForSelector('selector', { timeout: 15000 }).catch(() => null);

// WebView Bridge（想定）
{
  "timeout": 15000,
  "ignoreErrors": true
}
```

### 要素が見つからない場合
```javascript
// Puppeteer
const el = document.querySelector('div.price');
const price = el?.innerText || 'N/A';

// WebView Bridge（想定）
{
  "selector": "div.price",
  "default": "N/A"
}
```

### 複数要素の取得
```javascript
// Puppeteer
const elements = document.querySelectorAll('div.item');
const results = Array.from(elements).map(el => el.innerText);

// WebView Bridge（想定）
{
  "selector": "div.item",
  "extractAll": true,
  "limit": 20
}
```

---

## セッション管理

### Cookie保存
```javascript
// Puppeteer
const cookies = await page.cookies();
fs.writeFileSync('cookies.json', JSON.stringify(cookies, null, 2));

// 次回起動時
const cookies = JSON.parse(fs.readFileSync('cookies.json'));
await page.setCookie(...cookies);
```

### ユーザーデータディレクトリ
```javascript
// Puppeteer
const PROFILE_PATH = '/home/mhit/.config/chrome-automation';

await puppeteer.launch({
  args: [`--user-data-dir=${PROFILE_PATH}`]
});
```

### WebView Bridgeでの実装
- Cookie/LocalStorageの永続化APIが必要
- ユーザーデータディレクトリを指定する機能
- 複数サイトのセッションを独立して管理

---

## パフォーマンス最適化

### Puppeteerでの最適化
```javascript
// ヘッドレスモード
headless: true

// 不要なリソースをブロック
await page.setRequestInterception(true);
page.on('request', (req) => {
  if (['image', 'stylesheet', 'font'].includes(req.resourceType())) {
    req.abort();
  } else {
    req.continue();
  }
});

// ユーザーエージェント設定
await page.setUserAgent('Mozilla/5.0 ...');

// キャッシュ有効化
args: ['--disk-cache-dir=/path/to/cache']
```

---

## まとめ

### WebView Bridge実装時のポイント
1. **セレクター戦略**: `data-testid` を優先
2. **エラーハンドリング**: タイムアウト・要素不在に対応
3. **セッション管理**: Cookie/LocalStorageの永続化
4. **パフォーマンス**: ヘッドレスモード、リソースブロック
5. **API設計**: Puppeteerの主要操作をHTTPでエクスポーズ

### 次ステップ
1. TEST_CASES.md のテストケースを実装
2. 現行スクリプトとのAPIマッピング確認
3. パフォーマンステスト
4. 実運用への移行

---

**作成日:** 2026-02-05
**作成者:** Sam (OpenClaw Assistant)
**用途:** WebView Bridge実装参考資料
