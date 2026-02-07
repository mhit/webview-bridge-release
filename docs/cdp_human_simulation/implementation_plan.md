# Implementation Plan: CDP Human Mouse Simulation

## 1. Core/WebView拡張
CDP入力コマンドに `human_mode` フラグを追加し、シミュレーションロジックを実装します。

### 1.1 `WebViewInstance::click_cdp` の拡張
- **引数追加**: `human_mode: bool`
- **ロジック**:
  - `human_mode=true` の場合:
    - 現在位置からターゲット座標までの距離を計算
    - 3次ベジエ曲線制御点を生成（垂直オフセットによる自然な弧）
    - 距離に応じたステップ数（20〜60ステップ）を算出
    - イーズイン・アウト関数でt値を変換
    - ループ内で `Input.dispatchMouseEvent` (mouseMoved) を送信
    - 各ステップにマイクロジッター（終点に近づくほど減少）を追加
    - 10%の確率でオーバーシュート動作を挿入
    - クリック前後にランダムな遅延を追加
  - **イベント**: `mouseMoved` -> `mousePressed` -> `mouseReleased`

### 1.2 `WebViewInstance::type_cdp` の拡張
- **引数追加**: `human_mode: bool`
- **ロジック**:
  - `human_mode=true` の場合:
    - QWERTY配列に基づく隣接キーマップを定義
    - 文字ごとに確率的Typo発生（3%）
      - 間違ったキーを入力 -> 一時停止 -> Backspace -> 正しいキー
    - シフトキーミス（1.5% Shift忘れ, 1% シフト押しすぎ）
    - 文字種別（句読点、数字、スペース、頻出ペア）に応じた `delay` 計算
    - 稀に思考停止（2%）による長めのポーズ

## 2. コマンドフローの更新
MCPツールからWebViewインスタンスまで `human_mode` フラグを伝搬させます。

- `mcp_v3::tools::mcp_tool_handler`: ツール引数から `options.human_mode` を取得
- `core::AppCommand::ClickCdp/TypeCdp`: フィールド追加
- `core::SessionCommand::ClickCdp/TypeCdp`: フィールド追加
- `core::CoreManager::click_cdp/type_cdp`: 引数追加
- `main.rs`: コマンドハンドリング更新

## 3. クリーンアップ
JS実装 (`robustness.rs`) から不要になったシミュレーション関数を削除します。
- `generate_human_mouse_move_script`
- `generate_type_with_events_script`
- `generate_type_with_events_script_ex`

## 検証計画
1. **ビルド**: Rustコンパイラによる静的解析
2. **動作確認**:
   - `human_mode: true` でクリック時のマウスカーソルが曲線を描くか
   - 入力時にTypoと修正が発生するか
   - 入力速度が一定でないか
3. **リグレッション**:
   - `human_mode: false` での高速動作が維持されているか
