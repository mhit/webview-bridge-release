//! Visual Interactivity Analysis Module
//!
//! Analyzes DOM elements to determine their interactivity level
//! using a combination of rule-based scoring and LLM analysis.

use serde::{Deserialize, Serialize};

// Note: These structs define the expected JSON structure but are not directly
// instantiated in Rust. They serve as documentation and for future typed parsing.
#[allow(dead_code)]

/// Element with interactivity analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzedElement {
    pub index: usize,
    pub tag: String,
    #[serde(rename = "type")]
    pub element_type: Option<String>,
    pub selector: String,
    pub label: Option<String>,
    pub value: Option<String>,
    pub in_viewport: bool,
    pub visual: VisualProperties,
    pub interactivity: InteractivityAnalysis,
}

/// Visual properties extracted from computed styles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualProperties {
    pub cursor: String,
    #[serde(rename = "backgroundColor")]
    pub background_color: String,
    #[serde(rename = "borderRadius")]
    pub border_radius: String,
    #[serde(rename = "boxShadow")]
    pub box_shadow: Option<String>,
    #[serde(rename = "hasTransition")]
    pub has_transition: bool,
    #[serde(rename = "hasIcon")]
    pub has_icon: bool,
    #[serde(rename = "hasImage")]
    pub has_image: bool,
    #[serde(rename = "imageAlt")]
    pub image_alt: Option<String>,
    pub size: ElementSize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementSize {
    pub width: u32,
    pub height: u32,
}

/// Interactivity analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractivityAnalysis {
    pub score: f32,
    pub reasons: Vec<String>,
    pub predicted_actions: Vec<PredictedAction>,
    pub needs_vision: bool,
    pub analyzed_by: String, // "rule" or "llm" or "rule_only"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_reasoning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictedAction {
    pub action: String,
    pub purpose: String,
}

/// Analyze elements with LLM for more accurate interactivity scoring
/// Only analyzes elements with mid-range scores (0.2-0.7) where rule-based
/// scoring may be insufficient
pub fn analyze_with_llm(
    elements_json: &str,
    ai_config: &crate::core::ai::AiConfig,
) -> Result<String, String> {
    // Parse elements
    let parsed: serde_json::Value = serde_json::from_str(elements_json)
        .map_err(|e| format!("Failed to parse elements JSON: {}", e))?;

    let elements = parsed
        .get("elements")
        .and_then(|e| e.as_array())
        .ok_or("No elements array found")?;

    // Filter elements that need LLM analysis (mid-range scores)
    let needs_analysis: Vec<&serde_json::Value> = elements
        .iter()
        .filter(|el| {
            let score = el
                .get("interactivity")
                .and_then(|i| i.get("score"))
                .and_then(|s| s.as_f64())
                .unwrap_or(0.0);
            // Analyze elements with scores 0.3-0.8 (uncertain range)
            // With position bonuses, scores shifted higher
            score >= 0.3 && score < 0.8
        })
        .take(10) // Limit to 10 elements per batch
        .collect();

    if needs_analysis.is_empty() {
        // No elements need LLM analysis, return original
        return Ok(elements_json.to_string());
    }

    // Build prompt for LLM
    let prompt = build_analysis_prompt(&needs_analysis);

    // Call LLM based on provider
    let llm_response = match ai_config.provider.to_lowercase().as_str() {
        "ollama" => {
            let client = crate::core::ai::OllamaClient::new(ai_config);
            client.call(&prompt, None)?
        }
        "gemini" => {
            let client = crate::core::ai::GeminiClient::new(ai_config)
                .ok_or("Gemini client not available (missing API key?)")?;
            client.call(&prompt, None)?
        }
        _ => {
            return Err(format!("Unsupported AI provider: {}", ai_config.provider));
        }
    };

    // Parse LLM response and update elements
    let updated_elements = apply_llm_analysis(elements_json, &llm_response)?;

    Ok(updated_elements)
}

/// Build a prompt for LLM to analyze visual interactivity
fn build_analysis_prompt(elements: &[&serde_json::Value]) -> String {
    let mut prompt = String::from(
        r#"以下のWeb要素のインタラクティビティ（操作可能性）を分析してください。

各要素について:
1. score: 0.0-1.0のスコア（1.0が最もクリック可能）
2. action: 予測されるアクション（click_navigate, click_submit, type_input, click_toggle, click_expand, hover_reveal等）
3. reason: 判断理由（1文）

JSON形式で回答:
{"results": [{"index": 1, "score": 0.85, "action": "click_submit", "reason": "送信ボタンの特徴"}]}

要素リスト:
"#,
    );

    for el in elements {
        let index = el.get("index").and_then(|i| i.as_u64()).unwrap_or(0);
        let tag = el.get("tag").and_then(|t| t.as_str()).unwrap_or("?");
        let label = el.get("label").and_then(|l| l.as_str()).unwrap_or("");
        let visual = el.get("visual");
        let cursor = visual
            .and_then(|v| v.get("cursor"))
            .and_then(|c| c.as_str())
            .unwrap_or("");
        let has_transition = visual
            .and_then(|v| v.get("hasTransition"))
            .and_then(|t| t.as_bool())
            .unwrap_or(false);
        let has_image = visual
            .and_then(|v| v.get("hasImage"))
            .and_then(|i| i.as_bool())
            .unwrap_or(false);
        let bg = visual
            .and_then(|v| v.get("backgroundColor"))
            .and_then(|b| b.as_str())
            .unwrap_or("");
        let rule_score = el
            .get("interactivity")
            .and_then(|i| i.get("score"))
            .and_then(|s| s.as_f64())
            .unwrap_or(0.0);

        prompt.push_str(&format!(
            "\n{}. <{}> label=\"{}\" cursor={} transition={} image={} bg=\"{}\" ruleScore={:.2}",
            index,
            tag,
            label.chars().take(30).collect::<String>(),
            cursor,
            has_transition,
            has_image,
            bg.chars().take(30).collect::<String>(),
            rule_score
        ));
    }

    prompt.push_str("\n\nJSON:");
    prompt
}

/// Apply LLM analysis results to update element scores
fn apply_llm_analysis(original_json: &str, llm_response: &str) -> Result<String, String> {
    // Parse original
    let mut parsed: serde_json::Value = serde_json::from_str(original_json)
        .map_err(|e| format!("Failed to parse original JSON: {}", e))?;

    // Try to extract JSON from LLM response (may be wrapped in markdown)
    // Use char_indices to avoid UTF-8 boundary issues
    let json_start = llm_response
        .char_indices()
        .find(|(_, c)| *c == '{')
        .map(|(i, _)| i)
        .unwrap_or(0);
    let json_end = llm_response
        .char_indices()
        .rev()
        .find(|(_, c)| *c == '}')
        .map(|(i, _)| i + 1)
        .unwrap_or(llm_response.len());

    if json_start >= json_end {
        // No valid JSON found, return original unchanged
        eprintln!("[Visual Interactivity] No valid JSON in LLM response, using rule-based scores");
        return Ok(original_json.to_string());
    }

    let json_str = &llm_response[json_start..json_end];

    // Parse LLM response with fallback
    let llm_result: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "[Visual Interactivity] Failed to parse LLM response: {} - falling back to rule-based",
                e
            );
            return Ok(original_json.to_string());
        }
    };

    let results = match llm_result.get("results").and_then(|r| r.as_array()) {
        Some(r) => r,
        None => {
            eprintln!("[Visual Interactivity] No 'results' array in LLM response");
            return Ok(original_json.to_string());
        }
    };

    // Build index -> result map
    let mut result_map: std::collections::HashMap<u64, &serde_json::Value> =
        std::collections::HashMap::new();
    for result in results {
        if let Some(idx) = result.get("index").and_then(|i| i.as_u64()) {
            result_map.insert(idx, result);
        }
    }

    // Update elements
    if let Some(elements) = parsed.get_mut("elements").and_then(|e| e.as_array_mut()) {
        for element in elements.iter_mut() {
            if let Some(idx) = element.get("index").and_then(|i| i.as_u64()) {
                if let Some(llm_data) = result_map.get(&idx) {
                    // Update interactivity with LLM results
                    if let Some(interactivity) = element.get_mut("interactivity") {
                        if let Some(new_score) = llm_data.get("score").and_then(|s| s.as_f64()) {
                            interactivity["score"] = serde_json::json!(new_score);
                        }
                        if let Some(reason) = llm_data.get("reason").and_then(|r| r.as_str()) {
                            interactivity["llm_reasoning"] = serde_json::json!(reason);
                        }
                        if let Some(action) = llm_data.get("action").and_then(|a| a.as_str()) {
                            // Add LLM predicted action
                            if let Some(actions) = interactivity
                                .get_mut("predicted_actions")
                                .and_then(|a| a.as_array_mut())
                            {
                                actions.insert(
                                    0,
                                    serde_json::json!({
                                        "action": action,
                                        "purpose": "LLM分析"
                                    }),
                                );
                            }
                        }
                        interactivity["analyzed_by"] = serde_json::json!("llm");
                    }
                }
            }
        }
    }

    serde_json::to_string(&parsed).map_err(|e| format!("Failed to serialize updated JSON: {}", e))
}

/// Analyze elements that need vision analysis (images without alt text)
/// Takes element screenshots and sends to Vision LLM for content description
pub fn analyze_with_vision(
    elements_json: &str,
    ai_config: &crate::core::ai::AiConfig,
    capture_element_fn: impl Fn(&str) -> Result<String, String>, // Returns base64 image
) -> Result<String, String> {
    // Parse elements
    let mut parsed: serde_json::Value = serde_json::from_str(elements_json)
        .map_err(|e| format!("Failed to parse elements JSON: {}", e))?;

    // Collect elements that need vision analysis (avoid borrow issues)
    let needs_vision: Vec<(usize, String, String, f64)> = {
        let elements = parsed
            .get("elements")
            .and_then(|e| e.as_array())
            .ok_or("No elements array found")?;

        elements
            .iter()
            .enumerate()
            .filter(|(_, el)| {
                el.get("interactivity")
                    .and_then(|i| i.get("needs_vision"))
                    .and_then(|n| n.as_bool())
                    .unwrap_or(false)
            })
            .take(5) // Limit to 5 elements (vision is expensive)
            .map(|(idx, el)| {
                let selector = el
                    .get("selector")
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string();
                let tag = el
                    .get("tag")
                    .and_then(|t| t.as_str())
                    .unwrap_or("element")
                    .to_string();
                let score = el
                    .get("interactivity")
                    .and_then(|i| i.get("score"))
                    .and_then(|s| s.as_f64())
                    .unwrap_or(0.5);
                (idx, selector, tag, score)
            })
            .collect()
    };

    if needs_vision.is_empty() {
        return Ok(elements_json.to_string());
    }

    // Analyze each element with vision
    for (idx, selector, tag, current_score) in needs_vision {
        if selector.is_empty() {
            continue;
        }

        // Capture element screenshot
        let image_base64 = match capture_element_fn(&selector) {
            Ok(img) => img,
            Err(e) => {
                eprintln!("[Vision] Failed to capture element {}: {}", selector, e);
                continue;
            }
        };

        // Build vision prompt
        let prompt = build_vision_prompt_simple(&tag, current_score);

        // Call Vision LLM
        let vision_response = match ai_config.provider.to_lowercase().as_str() {
            "ollama" => {
                let client = crate::core::ai::OllamaClient::new(ai_config);
                client.call(&prompt, Some(&image_base64))
            }
            "gemini" => {
                let client = crate::core::ai::GeminiClient::new(ai_config)
                    .ok_or("Gemini client not available")?;
                client.call(&prompt, Some(&image_base64))
            }
            _ => Err(format!("Unsupported provider: {}", ai_config.provider)),
        };

        // Update element with vision results
        if let Ok(response) = vision_response {
            if let Some(elements_array) = parsed.get_mut("elements").and_then(|e| e.as_array_mut())
            {
                if let Some(element) = elements_array.get_mut(idx) {
                    // Parse vision response
                    let (description, predicted_action) = parse_vision_response(&response);

                    // Add vision_description to element
                    element["vision_description"] = serde_json::json!(description);

                    // Update label if it was empty
                    if element
                        .get("label")
                        .and_then(|l| l.as_str())
                        .unwrap_or("")
                        .is_empty()
                    {
                        element["label"] =
                            serde_json::json!(description.chars().take(50).collect::<String>());
                    }

                    // Add predicted action from vision
                    if let Some(action) = predicted_action {
                        if let Some(interactivity) = element.get_mut("interactivity") {
                            if let Some(actions) = interactivity
                                .get_mut("predicted_actions")
                                .and_then(|a| a.as_array_mut())
                            {
                                actions.insert(
                                    0,
                                    serde_json::json!({
                                        "action": action,
                                        "purpose": "Vision分析"
                                    }),
                                );
                            }
                            interactivity["analyzed_by"] = serde_json::json!("vision");
                            interactivity["needs_vision"] = serde_json::json!(false);
                        }
                    }
                }
            }
        }
    }

    serde_json::to_string(&parsed).map_err(|e| format!("Failed to serialize: {}", e))
}

/// Build prompt for Vision LLM (simple version with direct params)
fn build_vision_prompt_simple(tag: &str, current_score: f64) -> String {
    format!(
        r#"この{}要素の画像を分析してください。

1. 画像の内容を簡潔に説明（20文字以内）
2. クリック可能性を判断（0.0-1.0、現在のスコア: {:.2}）
3. 予測されるアクション（click_product, click_navigate, click_banner等）

JSON形式で回答:
{{"description": "商品画像", "score": 0.8, "action": "click_product"}}
"#,
        tag, current_score
    )
}

/// Parse vision LLM response
fn parse_vision_response(response: &str) -> (String, Option<String>) {
    // Try to extract JSON
    let json_start = response.find('{').unwrap_or(0);
    let json_end = response.rfind('}').map(|i| i + 1).unwrap_or(response.len());

    if json_start >= json_end {
        return (response.chars().take(50).collect(), None);
    }

    let json_str = &response[json_start..json_end];

    match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(parsed) => {
            let description = parsed
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("画像")
                .to_string();
            let action = parsed
                .get("action")
                .and_then(|a| a.as_str())
                .map(|s| s.to_string());
            (description, action)
        }
        Err(_) => {
            // Fallback: use raw response as description
            (response.chars().take(50).collect(), None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt() {
        let elements = vec![serde_json::json!({
            "index": 1,
            "tag": "button",
            "label": "購入する",
            "visual": {
                "cursor": "pointer",
                "hasTransition": true,
                "hasImage": false,
                "backgroundColor": "rgb(255, 153, 0)"
            },
            "interactivity": {
                "score": 0.5
            }
        })];
        let refs: Vec<&serde_json::Value> = elements.iter().collect();
        let prompt = build_analysis_prompt(&refs);

        assert!(prompt.contains("購入する"));
        assert!(prompt.contains("pointer"));
        assert!(prompt.contains("0.50"));
    }
}
