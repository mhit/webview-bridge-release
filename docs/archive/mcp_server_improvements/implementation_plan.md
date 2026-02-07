# WebView Bridge MCP サーバー改善計画

## 概要
MCPサーバーの問題点を修正し、機能を拡張する。

## 実装済み機能

### 1. ✅ 認証状態チェック (`auth_status`) の改善
**問題**: `auth_status.checked_at` が常に null、`logged_in` が実際の状態と異なる

**実装内容**:
- `check_auth` MCP ツールを新規追加
  - 汎用ログインインジケーター（ログアウトボタン、ユーザーメニュー等）の検出
  - カスタムセレクタ（`logged_in_selector`, `logged_out_selector`）のサポート
  - ユーザー名の自動抽出
- `session_acquire` で既存セッションの `auth_status` を返すように修正
- **サイト固有の検出** (2026-02-06追加):
  - X/Twitter: `SideNav_AccountSwitcher_Button`, タイムライン検出
  - Google: アカウントアバター, サインアウトリンク
  - Yahoo Japan: `yHeader-logout`, `yHeader-user`
  - Rakuten: `rex-header-myrakuten`, マイ楽天リンク
  - Facebook: アカウント設定, プロフィールリンク
  - Amazon: `nav-link-accountList`, サインアウト
  - Instagram: 設定アイコン, ログアウトリンク

### 2. ✅ Cookie 管理機能の追加
**実装内容**:
- `get_cookies` - セッションの Cookie を取得
- `set_cookies` - Cookie を設定（JSON配列形式）

### 3. ✅ スクリーンショット機能の完全改善
**問題**: 非headlessモードでSPAサイト（X等）でスクリーンショット失敗（CSP制限）

**実装内容**:
1. **html2canvas 方式（プライマリ）**
   - CSP回避のため `fetch+eval` 方式を追加
   - 複数CDNフォールバック（cdnjs, jsdelivr, unpkg）
   - ステータス追跡の改善（`__wbp_ss_status` で状態管理）
   - タイムアウト延長（10秒→15秒）

2. **WebView2 ネイティブ CapturePreview API（フォールバック）** ★新規追加
   - html2canvas がCSPでブロックされた場合に自動フォールバック
   - `ICoreWebView2::CapturePreview` を使用
   - CSPに影響されないネイティブAPIによるキャプチャ
   - PNG形式で取得、Base64エンコードして返却

**実装詳細**:
- `webview_instance.rs` に `CapturePreviewHandler` コールバックハンドラを追加
- `capture_preview_native()` メソッドで IStream へのキャプチャ実行
- `read_stream_to_vec()` ヘルパー関数で IStream からバイト列を読み取り
- `core/mod.rs` の Screenshot コマンドで html2canvas 失敗時に自動フォールバック

### 4. ✅ `goal_extract` セレクタ推論
**問題**: セレクタが未指定だと機能しない

**実装内容**:
- セレクタ未指定時にページ構造を自動分析
- 説明文に基づくセレクタ優先度設定（tweet, product, image 等）
- 複数のセレクタ戦略（リスト、テーブル、検索結果、メインコンテンツ）
- 推論されたセレクタを結果に含めて返却
- フォールバックとしてページ全体のテキストを返却

## 変更ファイル

| ファイル | 変更内容 |
|---------|---------|
| `src/api_v2.rs` | `check_auth`, `get_cookies`, `set_cookies` ツール追加、`session_acquire` 改善、`goal_extract` 改善、サイト固有検出追加 |
| `src/core/mod.rs` | Screenshot コマンド処理の改善、ネイティブ CapturePreview フォールバック追加 |
| `src/webview/webview_instance.rs` | `CapturePreviewHandler`、`capture_preview_native()`、`read_stream_to_vec()` 追加 |

## テスト結果 (2026-02-06)

| テスト項目 | 結果 | 詳細 |
|-----------|------|------|
| set_cookies | ✅ 成功 | WSL側CookieをWebViewに設定 |
| browse | ✅ 成功 | Xホームへ移動 |
| check_auth | ✅ 改善完了 | X固有セレクタ追加済み |
| goal_extract | ✅ 成功 | `[data-testid="tweet"]` を推論 |
| screenshot | ✅ 改善完了 | ネイティブAPIフォールバック実装 |

## 新規実装機能 (2026-02-06)

### 5. ✅ YouTube メディア機能

**実装内容**:

#### 5.1 `youtube_subtitles` - 字幕抽出
- yt-dlp を使用した YouTube 字幕取得
- 言語指定（`language`）: `ja`, `en` など
- 出力フォーマット: `text`（タイムスタンプ除去）、`srt`、`vtt`
- 自動生成字幕サポート（`auto_generated`）

#### 5.2 `youtube_download` - 動画/音声ダウンロード
- yt-dlp を使用した動画ダウンロード
- 品質指定: `best`, `hd`(1080p), `sd`(720p), `low`
- 音声のみ抽出（`audio_only`）→ MP3変換
- メタデータ自動埋め込み

**依存関係**:
- `yt-dlp` (Python: `pip install yt-dlp`)
- 実行方式: `python -m yt_dlp`（PATH問題回避）

### 6. ✅ 画像一括収集機能

**実装内容**:

#### 6.1 `collect_images` - ページ画像収集
- ページ内の全画像URLを取得
- フィルタリング:
  - `selector`: コンテナセレクタ（デフォルト: `body`）
  - `min_width`, `min_height`: 最小サイズフィルタ
  - `max_images`: 最大取得数
- ダウンロードオプション（`download: true`）
  - ローカルファイルに保存
  - 拡張子自動判定（URLパラメータ除去）

### 7. ⚠️ ジョブ管理（フレームワークのみ）

#### 7.1 `job_status` - ジョブ状態確認
- 長時間実行ジョブの進捗確認
- 現在はプレースホルダー実装
- 将来的に非同期ダウンロードの進捗追跡に使用予定

## 変更ファイル（追加）

| ファイル | 変更内容 |
|---------|---------|
| `src/api_v2.rs` | `youtube_subtitles`, `youtube_download`, `collect_images`, `job_status` ツール実装 |
| `src/mcp_stdio.rs` | 同上ツール定義・プロキシ呼び出し追加 |

## 未実装機能

| 機能 | 状態 | 備考 |
|------|------|------|
| WebView2 ダウンロードイベント連携 | ❌ 未実装 | ブラウザ内ダウンロード完了検知 |
| 非同期ジョブキュー | ❌ 未実装 | 長時間ダウンロードの進捗追跡 |
| FFmpeg 動画分析 | ❌ 未実装 | シーン検出、キーフレーム抽出 |
| バッチダウンロード | ❌ 未実装 | 複数URL並列ダウンロード |

