# CDP統合 実装計画 v3

## 現状分析

### 現在の実装レイヤー

```
┌─────────────────────────────────────────────────────────┐
│ MCP Tools (tools.rs)                                   │
│ - navigate, interact, capture, extract, session, ...   │
├─────────────────────────────────────────────────────────┤
│ Action Execution (tools.rs)                            │
│ - execute_action() → JavaScript生成 → execute_script() │
├─────────────────────────────────────────────────────────┤
│ WebViewInstance (webview_instance.rs)                  │
│ - click(), type_text() → JavaScript経由               │
│ - get_cookies() → document.cookie                      │
│ - screenshot() → canvas/html2canvas                    │
│ - set_user_agent() → Object.defineProperty             │
│ - simulate_device() → CDP 🔵 (既にCDP)                   │
├─────────────────────────────────────────────────────────┤
│ CDP / WebView2 API                                      │
└─────────────────────────────────────────────────────────┘
```

### 既にCDPを使用している機能

| 関数 | CDP Command | 状態 |
|------|-------------|------|
| `simulate_device()` | `Emulation.setDeviceMetricsOverride` | ✅ CDP使用中 |
| `simulate_device()` | `Emulation.setTouchEmulationEnabled` | ✅ CDP使用中 |
| `simulate_device()` | `Emulation.setUserAgentOverride` | ✅ CDP使用中 |
| `reset_device_emulation()` | `Emulation.clearDeviceMetricsOverride` | ✅ CDP使用中 |
| `set_viewport_cdp()` | `Emulation.setDeviceMetricsOverride` | ✅ CDP使用中 |
| `capture_screenshot_cdp()` | `Page.captureScreenshot` | ✅ CDP使用中 |

### JavaScript経由の機能（CDP置換対象）

| 関数 | 現在の実装 | 問題点 |
|------|-----------|--------|
| `get_cookies()` | `document.cookie` | HttpOnly取得不可 |
| `set_cookie()` | `document.cookie=` | HttpOnly設定不可 |
| `get_cookies_json()` | `document.cookie` | HttpOnly取得不可 |
| `set_cookies_json()` | JavaScript | HttpOnly設定不可 |
| `set_user_agent()` | `Object.defineProperty` | ネットワークレベルで偽装されない |
| `click()` | `element.click()` | 低レベルで使用されない（interactが使用） |
| `type_text()` | `el.value = '...'` | 低レベルで使用されない（interactが使用） |
| `screenshot()` | canvas生成 | 簡易実装、フルページ不可 |

### MCPから呼ばれる実際のフロー

```
[interact tool] 
  → handle_interact() 
    → execute_action() 
      → generate_scroll_and_click_script() → JavaScript生成
      → generate_type_with_events_script_ex() → JavaScript生成
      → execute_script() → WebView ExecuteScript API
```

**注意**: `interact` ツールは `WebViewInstance.click()` を**使用していない**
代わりに独自のJavaScriptをその場で生成して実行している

---

## 方針

### ✅ CDP置換すべき機能

| 対象 | 理由 | 優先度 |
|------|------|--------|
| Cookie管理 (`get_cookies`, `set_cookies`) | HttpOnly対応必須 | 🔴 高 |
| User-Agent (`set_user_agent`) | ネットワークレベル偽装 | 🔴 高 |
| スクリーンショット (`screenshot`) | 既存CDPをデフォルト化 | ✅ 完了 |

### 🟡 CDP置換を検討すべき機能

| 対象 | 理由 | 優先度 |
|------|------|--------|
| interact のクリック/入力 | ボット検出回避向上 | 🟡 中 |
| ネットワーク監視 | API傍受機能追加 | 🟡 中 |

### ❌ CDP置換不要な機能

| 対象 | 理由 |
|------|------|
| `click()`, `type_text()` in webview_instance.rs | interact経由で使用されない |
| `simulate_device()` | 既にCDP使用中 |
| `reset_device_emulation()` | 既にCDP使用中 |

---

## Phase 1: Cookie管理 (2時間)

### 目的
セッション維持に必要なHttpOnly cookieを取得・設定可能にする

### 置換対象

**Before (JavaScript):**
```javascript
document.cookie.split(';').map(...)
```

**After (CDP):**
```
Network.getCookies
Network.setCookie
Network.deleteCookies
```

### 実装内容

#### 1.1 `get_cookies()` 置換
```rust
// webview_instance.rs
pub fn get_cookies(&self) -> WinResult<Vec<CookieInfo>> {
    // CDP: Network.getCookies を使用
    // HttpOnly, Secure, SameSite, Expires 等も取得可能
}
```

#### 1.2 `set_cookie()` 置換
```rust
pub fn set_cookie(&self, cookie: CookieInfo) -> Result<(), String> {
    // CDP: Network.setCookie を使用
}
```

#### 1.3 `CookieInfo` 構造体拡張
```rust
pub struct CookieInfo {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub expires: Option<f64>,      // 追加
    pub http_only: Option<bool>,   // 追加
    pub secure: Option<bool>,      // 追加
    pub same_site: Option<String>, // 追加
}
```

### 成果物
- [ ] `CookieInfo` 構造体拡張
- [ ] `get_cookies()` をCDP実装に置換
- [ ] `set_cookie()` をCDP実装に置換
- [ ] `get_cookies_json()` をCDP実装に置換
- [ ] `set_cookies_json()` をCDP実装に置換
- [ ] テスト: HttpOnly cookie取得確認

---

## Phase 2: User-Agent (1時間)

### 目的
ネットワークレベルでUser-Agentを偽装し、ボット検出を回避

### 現状
- `simulate_device()`: `Emulation.setUserAgentOverride` 使用 ✅
- `set_user_agent()`: JavaScript使用 ❌

### 実装内容
`set_user_agent()` を `Emulation.setUserAgentOverride` に置換

```rust
pub fn set_user_agent(&self, user_agent: &str) -> WinResult<()> {
    // CDP: Emulation.setUserAgentOverride を使用
    // simulate_device() と同じ方法
}
```

### 成果物
- [ ] `set_user_agent()` をCDP実装に置換
- [ ] テスト: httpbin.org/headers でUA確認

---

## Phase 3: スクリーンショット統合 (30分)

### 目的
CDPスクリーンショットをデフォルト化し、レガシーコードを削除

### 現状
- `screenshot()`: canvas（簡易実装）
- `capture_screenshot_cdp()`: CDP（フルページ対応）✅
- `capture_preview_native()`: CapturePreview API

### 実装内容
1. `screenshot()` の内部実装を `capture_screenshot_cdp()` のコードに置換
2. `capture_screenshot_cdp()` を削除
3. `capture_preview_native()` を削除

### 成果物
- [ ] `screenshot()` をCDP実装に統合
- [ ] `capture_screenshot_cdp()` 削除
- [ ] `capture_preview_native()` 削除

---

## Phase 4: interact入力イベント強化 (オプション, 3時間)

### 目的
interactツールのクリック・入力をCDPに置換してボット検出を回避

### 現状
`execute_action()` がJavaScriptを生成して実行:
- `generate_scroll_and_click_script()` → JS click
- `generate_type_with_events_script_ex()` → JS keyboard events

### 実装内容
オプションで `use_cdp_input: true` 時にCDP入力を使用:
- `Runtime.evaluate` で座標取得
- `Input.dispatchMouseEvent` でクリック
- `Input.dispatchKeyEvent` でキー入力

### 成果物
- [ ] CDP入力実装
- [ ] interactにオプション追加
- [ ] テスト

---

## Phase 5: ネットワーク監視 (新機能, 3時間)

### 目的
XHR/fetchの傍受でAPIレスポンスを直接取得

### 新機能
- `Network.enable` でモニタリング開始
- リクエスト/レスポンスログ取得
- `network` MCPツール追加

### 成果物
- [ ] ネットワーク監視実装
- [ ] `network` MCPツール追加
- [ ] テスト

---

## 優先度まとめ

| Phase | 内容 | 優先度 | 工数 | 効果 | ステータス |
|-------|------|--------|------|------|------------|
| 1 | Cookie管理 | 🔴 高 | 2h | ログイン維持確実 | ✅ 完了 |
| 2 | User-Agent | 🔴 高 | 1h | ボット検出回避 | ✅ 完了 |
| 3 | スクリーンショット統合 | 🟢 低 | 30min | コード整理 | ✅ 完了 |
| 4 | 入力イベント | 🟡 中 | 3h | ボット検出回避強化 | ✅ 完了 |
| 5 | ネットワーク監視 | 🟡 中 | 3h | 新機能 | 📋 未着手 |

**完了: Phase 1, 2, 3, 4**
**残り: Phase 5 (オプション)**

---

## 削除予定コード

Phase完了後に削除:
- `capture_screenshot_cdp()` → `screenshot()` に統合
- `set_viewport_cdp()` → `set_viewport()` に統合してもOK
- `capture_preview_native()` → 削除
- JavaScript経由のcookie操作 → 削除
- JavaScript経由のUA偽装 → 削除

---

## 次のアクション

**Phase 1 (Cookie管理) 開始:**
1. `CookieInfo` 構造体に `http_only`, `secure`, `same_site`, `expires` 追加
2. `get_cookies()` を `Network.getCookies` で置換
3. `set_cookie()` を `Network.setCookie` で置換
4. テスト
