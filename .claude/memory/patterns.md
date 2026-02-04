# Patterns

> プロジェクトでよく使われるパターンと慣習の記録

---

## 🦀 Rust パターン

### エラーハンドリング

```rust
// Result 型でのエラー伝播
pub async fn navigate_to(url: &str) -> Result<()> {
    let parsed = Url::parse(url)?;  // ? でエラーを伝播
    // ...
    Ok(())
}
```

### 非同期関数

```rust
// Tokio ランタイムを使用
#[tokio::main]
async fn main() {
    // ...
}

// 非同期テスト
#[tokio::test]
async fn test_async_function() {
    // ...
}
```

### Arc + Mutex for 共有状態

```rust
use std::sync::Arc;
use tokio::sync::Mutex;

// スレッドセーフな共有状態
let sessions: Arc<Mutex<HashMap<SessionId, Session>>> =
    Arc::new(Mutex::new(HashMap::new()));
```

---

## 🔧 Axum パターン

### エンドポイント定義

```rust
use axum::{
    extract::Path,
    Json,
    http::StatusCode,
    response::IntoResponse,
};

// パスパラメータ付き
pub async fn get_session(
    Path(id): Path<String>,
) -> impl IntoResponse {
    // ...
}

// JSON リクエスト/レスポンス
pub async fn create_session(
    Json(req): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    // ...
}
```

### エラーレスポンス

```rust
pub enum ApiError {
    NotFound(String),
    InternalError(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}
```

---

## 🖥️ Windows API パターン

### COM 初期化

```rust
use windows::core::HRESULT;

#[tokio::main]
async fn main() -> Result<()> {
    unsafe {
        let hr = CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED);
        if FAILED(hr) {
            return Err(Error::ComInitFailed(hr));
        }
    }
    // ...
    Ok(())
}
```

### ウィンドウクラス登録

```rust
use windows::Win32::UI::WindowsAndMessaging::*;

unsafe fn register_window_class() -> Result<Atom> {
    let wc = WNDCLASSW {
        hInstance: GetModuleHandleW(None)?,
        lpszClassName: w!("WebViewBridgeClass"),
        lpfnWndProc: Some(window_proc),
        ..Default::default()
    };

    let atom = RegisterClassW(&wc)?;
    if atom == 0 {
        return Err(Error::WindowClassRegistrationFailed);
    }
    Ok(atom)
}
```

---

## 🧪 テストパターン

### モックとスタブ

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct MockWebView {
        navigated_urls: Arc<Mutex<Vec<String>>>,
    }

    impl MockWebView {
        fn new() -> Self {
            Self {
                navigated_urls: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }
}
```

### 統合テスト

```rust
// tests/integration_test.rs
use reqwest::Client;

#[tokio::test]
async fn test_session_lifecycle() {
    let client = Client::new();
    let base_url = "http://localhost:3000";

    // セッション作成
    let response = client
        .post(&format!("{}/api/sessions", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 201);
}
```

---

## 📦 プロジェクト構造

### モジュール分割

```
src/
├── main.rs              # エントリーポイント
├── api.rs               # HTTP API ハンドラー
├── core/
│   └── mod.rs           # コア機能（SessionManager など）
└── webview/
    ├── mod.rs           # WebView モジュール
    ├── webview_instance.rs
    └── window.rs        # Win32 ウィンドウ
```

### テスト配置

```
tests/
└── integration_test.rs  # 統合テスト

src/
└── lib.rs               # ユニットテストは各ファイル内に
```

---

## 🔄 よくある操作

### 新しい依存関係を追加

1. `Cargo.toml` に依存関係を追加
2. `cargo build` で確認
3. ドキュメントを確認: `cargo doc --open`

### 新しいエンドポイントを追加

1. 関数を `api.rs` に定義
2. ルーターに登録: `router.post("/api/endpoint", handler)`
3. テストを追加

### デバッグ

```bash
# ログ有効
RUST_LOG=debug cargo run

# バックトレース付き
RUST_BACKTRACE=1 cargo run
```

---

（以降、新しいパターンを追記してください）
