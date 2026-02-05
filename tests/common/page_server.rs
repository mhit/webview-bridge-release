//! Static File Server for E2E Tests
//!
//! Serves test HTML pages for E2E workflow tests.

use axum::{
    Router,
    routing::get,
    response::{Html, IntoResponse},
    http::StatusCode,
};
use tokio::net::TcpListener;

/// Start a static file server for test pages
pub async fn start_test_page_server(port: u16) -> tokio::task::JoinHandle<()> {
    let app = Router::new()
        .route("/", get(index_page))
        .route("/login.html", get(login_page))
        .route("/infinite_scroll.html", get(infinite_scroll_page))
        .route("/product_list.html", get(product_list_page))
        .route("/spa.html", get(spa_page))
        .route("/api/login", axum::routing::post(mock_login_api))
        .route("/api/products", get(mock_products_api));
    
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await
        .expect("Failed to bind test page server");
    
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    })
}

async fn index_page() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
<html>
<head><title>Test Site</title></head>
<body>
<h1>WBP2 Test Site</h1>
<ul>
    <li><a href="/login.html">Login Page</a></li>
    <li><a href="/infinite_scroll.html">Infinite Scroll</a></li>
    <li><a href="/product_list.html">Product List</a></li>
    <li><a href="/spa.html">SPA App</a></li>
</ul>
</body>
</html>"#)
}

async fn login_page() -> Html<&'static str> {
    Html(include_str!("../fixtures/pages/login.html"))
}

async fn infinite_scroll_page() -> Html<&'static str> {
    Html(include_str!("../fixtures/pages/infinite_scroll.html"))
}

async fn product_list_page() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
<html>
<head>
    <title>Product List</title>
    <style>
        body { font-family: Arial, sans-serif; padding: 20px; }
        .product { border: 1px solid #ddd; padding: 15px; margin: 10px 0; }
        .product-image { width: 100px; height: 100px; background: #eee; }
        .price { color: #c00; font-weight: bold; }
        .in-stock { color: green; }
        .out-of-stock { color: red; }
    </style>
</head>
<body>
<h1>Products</h1>
<div id="products">
    <div class="product" data-id="1">
        <div class="product-image"></div>
        <h3 class="product-name">Product A</h3>
        <p class="price">$99.99</p>
        <span class="in-stock">In Stock</span>
    </div>
    <div class="product" data-id="2">
        <div class="product-image"></div>
        <h3 class="product-name">Product B</h3>
        <p class="price">$149.99</p>
        <span class="in-stock">In Stock</span>
    </div>
    <div class="product" data-id="3">
        <div class="product-image"></div>
        <h3 class="product-name">Product C</h3>
        <p class="price">$199.99</p>
        <span class="out-of-stock">Out of Stock</span>
    </div>
</div>
<nav class="pagination">
    <a href="?page=1" class="active">1</a>
    <a href="?page=2">2</a>
    <a href="?page=3">3</a>
</nav>
</body>
</html>"#)
}

async fn spa_page() -> Html<&'static str> {
    Html(r##"<!DOCTYPE html>
<html>
<head>
    <title>SPA App</title>
    <style>
        body { font-family: Arial, sans-serif; padding: 20px; }
        .view { display: none; }
        .view.active { display: block; }
        nav a { margin-right: 15px; cursor: pointer; color: blue; }
        nav a.active { font-weight: bold; }
    </style>
</head>
<body>
<nav>
    <a href="#/" onclick="navigate('home')">Home</a>
    <a href="#/products" onclick="navigate('products')">Products</a>
    <a href="#/about" onclick="navigate('about')">About</a>
</nav>

<div id="home" class="view active">
    <h1>Welcome to SPA</h1>
    <p>This is a single-page application for testing.</p>
</div>

<div id="products" class="view">
    <h1>Products</h1>
    <div id="product-container"></div>
    <button id="load-more">Load More</button>
</div>

<div id="about" class="view">
    <h1>About Us</h1>
    <p>Test SPA for WBP2 E2E tests.</p>
</div>

<script>
let productCount = 0;

function navigate(viewId) {
    document.querySelectorAll('.view').forEach(v => v.classList.remove('active'));
    document.querySelectorAll('nav a').forEach(a => a.classList.remove('active'));
    document.getElementById(viewId).classList.add('active');
    history.pushState({view: viewId}, '', '#/' + viewId);
}

document.getElementById('load-more')?.addEventListener('click', function() {
    const container = document.getElementById('product-container');
    for (let i = 0; i < 5; i++) {
        productCount++;
        const div = document.createElement('div');
        div.className = 'product-item';
        div.innerHTML = `<p>Product ${productCount}</p>`;
        container.appendChild(div);
    }
});

window.onpopstate = function(e) {
    if (e.state && e.state.view) {
        navigate(e.state.view);
    }
};
</script>
</body>
</html>"##)
}

#[derive(serde::Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

async fn mock_login_api(
    axum::Json(req): axum::Json<LoginRequest>,
) -> impl IntoResponse {
    if req.username == "testuser" && req.password == "password123" {
        (StatusCode::OK, axum::Json(serde_json::json!({
            "success": true,
            "token": "mock-jwt-token-12345",
            "user": {
                "id": 1,
                "username": "testuser",
                "role": "user"
            }
        })))
    } else {
        (StatusCode::UNAUTHORIZED, axum::Json(serde_json::json!({
            "success": false,
            "error": "Invalid credentials"
        })))
    }
}

async fn mock_products_api() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "products": [
            {"id": 1, "name": "Product A", "price": 99.99, "in_stock": true},
            {"id": 2, "name": "Product B", "price": 149.99, "in_stock": true},
            {"id": 3, "name": "Product C", "price": 199.99, "in_stock": false}
        ],
        "total": 3,
        "page": 1
    }))
}

/// Get a unique port for the test page server
pub fn get_test_page_port() -> u16 {
    use std::sync::atomic::{AtomicU16, Ordering};
    static PORT_COUNTER: AtomicU16 = AtomicU16::new(19500);
    PORT_COUNTER.fetch_add(1, Ordering::SeqCst)
}
