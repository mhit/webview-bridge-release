//! AX Tree types for CDP Accessibility domain.
//!
//! These types map to the Chrome DevTools Protocol `Accessibility` domain.
//! `Accessibility.getFullAXTree` returns a flat list of `AXNode`s that form a tree via `childIds`.
//!
//! Reference: https://chromedevtools.github.io/devtools-protocol/tot/Accessibility/

use serde::{Deserialize, Serialize};

// ============================================================================
// CDP raw types (deserialized directly from getFullAXTree JSON)
// ============================================================================

/// A CDP AX value — wraps a typed value from the Accessibility domain.
/// Examples: `{"type":"role","value":"button"}`, `{"type":"boolean","value":true}`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AXValue {
    #[serde(rename = "type")]
    pub value_type: String,
    /// The actual value; may be string, bool, number, or absent for "none" type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    /// Related DOM nodes (used for labelledby, describedby, etc.)
    #[serde(
        rename = "relatedNodes",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub related_nodes: Vec<serde_json::Value>,
    /// Sources that contributed to this value (optional, for debugging).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<serde_json::Value>,
}

impl AXValue {
    /// Extract the inner value as a string, if available.
    pub fn as_str(&self) -> &str {
        match &self.value {
            Some(serde_json::Value::String(s)) => s.as_str(),
            _ => "",
        }
    }

    /// Extract the inner value as a bool, defaulting to false.
    pub fn as_bool(&self) -> bool {
        match &self.value {
            Some(serde_json::Value::Bool(b)) => *b,
            _ => false,
        }
    }

    /// Extract the inner value as a u64, if available.
    pub fn as_u64(&self) -> Option<u64> {
        match &self.value {
            Some(serde_json::Value::Number(n)) => n.as_u64(),
            _ => None,
        }
    }
}

/// A named AX property (e.g. `{"name":"disabled","value":{"type":"boolean","value":false}}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AXProperty {
    pub name: String,
    pub value: AXValue,
}

/// A single node in the CDP Accessibility tree.
///
/// Returned as elements of `nodes[]` from `Accessibility.getFullAXTree`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AXNode {
    /// CDP node ID (string integer) unique within the AX tree.
    #[serde(rename = "nodeId")]
    pub node_id: String,

    /// Whether this node is ignored (not exposed to AT).
    #[serde(default)]
    pub ignored: bool,

    /// Reasons why this node is ignored (only when ignored=true).
    #[serde(
        rename = "ignoredReasons",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub ignored_reasons: Vec<AXProperty>,

    /// The accessible role of this node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<AXValue>,

    /// The accessible name computed for this node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<AXValue>,

    /// The accessible description computed for this node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<AXValue>,

    /// The accessible value (for inputs, sliders, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<AXValue>,

    /// Additional properties (disabled, focused, checked, expanded, required, haspopup, level, etc.).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<AXProperty>,

    /// Child AX node IDs.
    #[serde(rename = "childIds", default, skip_serializing_if = "Vec::is_empty")]
    pub child_ids: Vec<String>,

    /// Backend DOM node ID — used to resolve the node back to a DOM element via CDP.
    #[serde(rename = "backendDOMNodeId", skip_serializing_if = "Option::is_none")]
    pub backend_dom_node_id: Option<i64>,

    /// Frame ID this node belongs to (for cross-origin iframes).
    #[serde(rename = "frameId", skip_serializing_if = "Option::is_none")]
    pub frame_id: Option<String>,

    /// Parent AX node ID (not always present in CDP response — populated by post-processing).
    #[serde(rename = "parentId", skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
}

impl AXNode {
    /// Convenience: role string value (e.g. "button", "link"), or "" if absent.
    pub fn role_str(&self) -> &str {
        self.role.as_ref().map(|v| v.as_str()).unwrap_or("")
    }

    /// Convenience: accessible name string, or "" if absent.
    pub fn name_str(&self) -> &str {
        self.name.as_ref().map(|v| v.as_str()).unwrap_or("")
    }

    /// Convenience: description string, or "" if absent.
    pub fn description_str(&self) -> &str {
        self.description.as_ref().map(|v| v.as_str()).unwrap_or("")
    }

    /// Convenience: value string (for textbox/select current value), or "" if absent.
    pub fn value_str(&self) -> &str {
        self.value.as_ref().map(|v| v.as_str()).unwrap_or("")
    }

    /// Look up a named property by name.
    pub fn get_property(&self, name: &str) -> Option<&AXProperty> {
        self.properties.iter().find(|p| p.name == name)
    }

    /// Get a boolean property, defaulting to false.
    pub fn bool_prop(&self, name: &str) -> bool {
        self.get_property(name)
            .map(|p| p.value.as_bool())
            .unwrap_or(false)
    }

    /// Get a u64 property (e.g. heading level).
    pub fn u64_prop(&self, name: &str) -> Option<u64> {
        self.get_property(name).and_then(|p| p.value.as_u64())
    }

    /// Get a string property (e.g. haspopup, autocomplete).
    pub fn str_prop<'a>(&'a self, name: &str) -> &'a str {
        self.get_property(name)
            .map(|p| p.value.as_str())
            .unwrap_or("")
    }

    /// Returns true if this node is interactive based on its AX role.
    pub fn is_interactive_role(&self) -> bool {
        matches!(
            self.role_str(),
            "button"
                | "link"
                | "textbox"
                | "checkbox"
                | "radio"
                | "combobox"
                | "listbox"
                | "menuitem"
                | "menuitemcheckbox"
                | "menuitemradio"
                | "option"
                | "switch"
                | "tab"
                | "treeitem"
                | "spinbutton"
                | "searchbox"
                | "slider"
        )
    }
}

/// The raw AX tree as returned by `Accessibility.getFullAXTree`.
/// This is a flat list of nodes; the tree structure is encoded via `childIds`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AXTree {
    pub nodes: Vec<AXNode>,
}

// ============================================================================
// Processed element (assigned a ref, ready for API response)
// ============================================================================

/// A resolved AX element with an assigned ref ID, ready for API/CLI output.
///
/// This is what callers receive after `get_ax_snapshot()` runs.
/// It contains the semantic information from the AX node plus:
/// - `ref_id`: the @eN shorthand assigned to interactive elements
/// - `is_cursor_interactive`: whether JS cursor/event detection found this interactive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AXElement {
    /// Stable ref assigned to this element (e.g. "e1"). Empty for non-interactive nodes.
    /// Serialized as "ref" to match DOM snapshot output.
    #[serde(rename = "ref")]
    pub ref_id: String,

    /// Accessible role (e.g. "button", "link", "textbox").
    pub role: String,

    /// Accessible name (computed label).
    pub name: String,

    /// Accessible description.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,

    /// Current value (for input, slider, select).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub value: String,

    /// Heading level (1-6), if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<u64>,

    /// Checked state (for checkbox, radio, switch).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,

    /// Expanded state (for combobox, treeitem, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded: Option<bool>,

    /// Whether the element is disabled.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,

    /// Whether the element currently has focus.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub focused: bool,

    /// Whether the element is required (for form fields).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub required: bool,

    /// haspopup value (e.g. "menu", "listbox", "tree").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub haspopup: Option<String>,

    /// CDP backend DOM node ID — used to click/interact via `DOM.focus`, `Input.*`.
    #[serde(rename = "backendNodeId", skip_serializing_if = "Option::is_none")]
    pub backend_node_id: Option<i64>,

    /// Frame ID this element belongs to (empty string = main frame).
    #[serde(rename = "frameId", skip_serializing_if = "String::is_empty")]
    pub frame_id: String,

    /// Whether this element was detected as interactive by JS cursor/event heuristics
    /// (CSS cursor:pointer, onclick, tabindex) even if not in an interactive AX role.
    #[serde(
        rename = "cursorInteractive",
        default,
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub is_cursor_interactive: bool,
}

impl AXElement {
    /// Build an AXElement from a raw AXNode, using its CDP data.
    /// `ref_id` is assigned by the caller (ref assignment logic).
    pub fn from_ax_node(node: &AXNode, ref_id: String) -> Self {
        let checked = if node.role_str() == "checkbox"
            || node.role_str() == "radio"
            || node.role_str() == "switch"
            || node.get_property("checked").is_some()
        {
            let prop = node.get_property("checked");
            prop.map(|p| p.value.as_bool())
        } else {
            None
        };

        let expanded = node.get_property("expanded").map(|p| p.value.as_bool());

        let haspopup_str = node.str_prop("haspopup");
        let haspopup = if haspopup_str.is_empty() || haspopup_str == "false" {
            None
        } else {
            Some(haspopup_str.to_string())
        };

        AXElement {
            ref_id,
            role: node.role_str().to_string(),
            name: node.name_str().to_string(),
            description: node.description_str().to_string(),
            value: node.value_str().to_string(),
            level: node.u64_prop("level"),
            checked,
            expanded,
            disabled: node.bool_prop("disabled"),
            focused: node.bool_prop("focused"),
            required: node.bool_prop("required"),
            haspopup,
            backend_node_id: node.backend_dom_node_id,
            frame_id: node.frame_id.clone().unwrap_or_default(),
            is_cursor_interactive: false,
        }
    }
}

// ============================================================================
// Snapshot result
// ============================================================================

/// The full AX snapshot result — returned by `get_ax_snapshot()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AXSnapshot {
    /// All interactive elements with assigned refs, in document order.
    pub elements: Vec<AXElement>,
    /// Total AX node count (including non-interactive / ignored).
    pub total_nodes: usize,
    /// Number of refs assigned.
    pub ref_count: usize,
}
