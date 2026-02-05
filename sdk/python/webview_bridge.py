"""
WebView Bridge Python SDK
A simple Python client for WebView Bridge server.

Usage:
    from webview_bridge import WebViewBridge
    
    bridge = WebViewBridge()  # Connect to localhost:9400
    
    # Create session
    session_id = bridge.create_session()
    
    # Navigate
    bridge.navigate(session_id, "https://example.com")
    
    # Execute JavaScript
    result = bridge.execute(session_id, "return document.title")
    
    # Take screenshot
    screenshot_base64 = bridge.screenshot(session_id)
    
    # Close session
    bridge.close(session_id)
"""

import requests
from typing import Optional, Any, Dict, List


class WebViewBridge:
    """WebView Bridge Python SDK Client"""
    
    def __init__(self, host: str = "localhost", port: int = 9400):
        self.base_url = f"http://{host}:{port}"
        self.session = requests.Session()
    
    def health(self) -> bool:
        """Check if the server is healthy."""
        try:
            response = self.session.get(f"{self.base_url}/health")
            return response.status_code == 200
        except Exception:
            return False
    
    def create_session(self, profile: str = "default", headless: bool = False) -> str:
        """Create a new browser session."""
        response = self.session.post(
            f"{self.base_url}/create",
            json={"profile": profile, "headless": headless}
        )
        response.raise_for_status()
        return response.json()["id"]
    
    def navigate(self, session_id: str, url: str) -> bool:
        """Navigate to a URL."""
        response = self.session.post(
            f"{self.base_url}/navigate/{session_id}",
            json={"url": url}
        )
        response.raise_for_status()
        return response.json().get("success", True)
    
    def execute(self, session_id: str, script: str) -> Any:
        """Execute JavaScript in the page context."""
        response = self.session.post(
            f"{self.base_url}/execute/{session_id}",
            json={"script": script}
        )
        response.raise_for_status()
        return response.json().get("result")
    
    def get_status(self, session_id: str) -> Dict[str, Any]:
        """Get session status."""
        response = self.session.get(f"{self.base_url}/status/{session_id}")
        response.raise_for_status()
        return response.json()
    
    def snapshot(self, session_id: str, format: str = "html") -> str:
        """Get page snapshot (HTML or text)."""
        response = self.session.get(
            f"{self.base_url}/snapshot/{session_id}",
            params={"format": format}
        )
        response.raise_for_status()
        return response.json().get("content", "")
    
    def screenshot(self, session_id: str) -> str:
        """Take a screenshot (returns base64 PNG)."""
        response = self.session.get(f"{self.base_url}/screenshot/{session_id}")
        response.raise_for_status()
        return response.json().get("image", "")
    
    def get_cookies(self, session_id: str) -> List[Dict[str, Any]]:
        """Get all cookies."""
        response = self.session.get(f"{self.base_url}/cookies/{session_id}")
        response.raise_for_status()
        return response.json().get("cookies", [])
    
    def set_cookies(self, session_id: str, cookies: List[Dict[str, Any]]) -> bool:
        """Set cookies."""
        response = self.session.post(
            f"{self.base_url}/cookies/{session_id}",
            json={"cookies": cookies}
        )
        response.raise_for_status()
        return True
    
    def wait_for_selector(
        self, session_id: str, selector: str, timeout_ms: int = 10000
    ) -> bool:
        """Wait for an element to appear."""
        response = self.session.post(
            f"{self.base_url}/wait/{session_id}",
            json={"selector": selector, "timeout_ms": timeout_ms}
        )
        response.raise_for_status()
        return response.json().get("found", False)
    
    def extract(
        self, session_id: str, selector: str, attribute: str = "text"
    ) -> List[str]:
        """Extract content from elements."""
        response = self.session.post(
            f"{self.base_url}/extract/{session_id}",
            json={"selector": selector, "attribute": attribute, "all": True}
        )
        response.raise_for_status()
        return response.json().get("results", [])
    
    def close(self, session_id: str) -> bool:
        """Close a session."""
        response = self.session.delete(f"{self.base_url}/close/{session_id}")
        response.raise_for_status()
        return True


class WebDriverBridge:
    """WebDriver Protocol Compatible Client (for Selenium compatibility)"""
    
    def __init__(self, host: str = "localhost", port: int = 9400):
        self.base_url = f"http://{host}:{port}/wd/hub"
        self.session = requests.Session()
    
    def new_session(self) -> str:
        """Create a new WebDriver session."""
        response = self.session.post(
            f"{self.base_url}/session",
            json={"capabilities": {}}
        )
        response.raise_for_status()
        return response.json()["value"]["sessionId"]
    
    def navigate(self, session_id: str, url: str) -> None:
        """Navigate to URL."""
        response = self.session.post(
            f"{self.base_url}/session/{session_id}/url",
            json={"url": url}
        )
        response.raise_for_status()
    
    def get_url(self, session_id: str) -> str:
        """Get current URL."""
        response = self.session.get(f"{self.base_url}/session/{session_id}/url")
        response.raise_for_status()
        return response.json()["value"]
    
    def get_title(self, session_id: str) -> str:
        """Get page title."""
        response = self.session.get(f"{self.base_url}/session/{session_id}/title")
        response.raise_for_status()
        return response.json()["value"]
    
    def find_element(self, session_id: str, using: str, value: str) -> str:
        """Find element by selector."""
        response = self.session.post(
            f"{self.base_url}/session/{session_id}/element",
            json={"using": using, "value": value}
        )
        response.raise_for_status()
        return response.json()["value"]["element-6066-11e4-a52e-4f735466cecf"]
    
    def find_elements(self, session_id: str, using: str, value: str) -> List[str]:
        """Find elements by selector."""
        response = self.session.post(
            f"{self.base_url}/session/{session_id}/elements",
            json={"using": using, "value": value}
        )
        response.raise_for_status()
        return [
            el["element-6066-11e4-a52e-4f735466cecf"]
            for el in response.json()["value"]
        ]
    
    def execute_script(self, session_id: str, script: str, args: List = None) -> Any:
        """Execute JavaScript."""
        response = self.session.post(
            f"{self.base_url}/session/{session_id}/execute/sync",
            json={"script": script, "args": args or []}
        )
        response.raise_for_status()
        return response.json()["value"]
    
    def screenshot(self, session_id: str) -> str:
        """Take screenshot."""
        response = self.session.get(
            f"{self.base_url}/session/{session_id}/screenshot"
        )
        response.raise_for_status()
        return response.json()["value"]
    
    def quit(self, session_id: str) -> None:
        """Quit session."""
        response = self.session.delete(f"{self.base_url}/session/{session_id}")
        response.raise_for_status()


class MCPBridge:
    """MCP (Model Context Protocol) Client"""
    
    def __init__(self, host: str = "localhost", port: int = 9400):
        self.base_url = f"http://{host}:{port}/mcp"
        self.session = requests.Session()
    
    def info(self) -> Dict[str, str]:
        """Get server info."""
        response = self.session.get(f"{self.base_url}/info")
        response.raise_for_status()
        return response.json()
    
    def list_tools(self) -> List[Dict[str, Any]]:
        """List available tools."""
        response = self.session.get(f"{self.base_url}/tools")
        response.raise_for_status()
        return response.json()
    
    def call_tool(self, name: str, arguments: Dict[str, Any]) -> Dict[str, Any]:
        """Call a tool."""
        response = self.session.post(
            f"{self.base_url}/tools/call",
            json={"name": name, "arguments": arguments}
        )
        response.raise_for_status()
        return response.json()
    
    def list_resources(self) -> List[Dict[str, Any]]:
        """List available resources."""
        response = self.session.get(f"{self.base_url}/resources")
        response.raise_for_status()
        return response.json()
    
    def read_resource(self, uri: str) -> Dict[str, Any]:
        """Read a resource."""
        response = self.session.post(
            f"{self.base_url}/resources/read",
            json={"uri": uri}
        )
        response.raise_for_status()
        return response.json()
    
    # Convenience methods
    def browse(self, url: str) -> str:
        """Navigate to URL."""
        result = self.call_tool("browse", {"url": url})
        return result["content"][0]["text"]
    
    def screenshot(self) -> str:
        """Take screenshot (base64)."""
        result = self.call_tool("screenshot", {})
        return result["content"][0]["data"]
    
    def extract(self, selector: str) -> str:
        """Extract text from selector."""
        result = self.call_tool("extract", {"selector": selector})
        return result["content"][0]["text"]
    
    def wait(self, selector: str, timeout: int = 10000) -> str:
        """Wait for element."""
        result = self.call_tool("wait", {"selector": selector, "timeout": timeout})
        return result["content"][0]["text"]
    
    def get_html(self) -> str:
        """Get page HTML."""
        result = self.call_tool("get_html", {})
        return result["content"][0]["text"]
    
    def get_text(self) -> str:
        """Get page text."""
        result = self.call_tool("get_text", {})
        return result["content"][0]["text"]


# Example usage
if __name__ == "__main__":
    # Test basic connectivity
    bridge = WebViewBridge()
    
    if bridge.health():
        print("✅ WebView Bridge is running!")
        
        # Create session
        session_id = bridge.create_session()
        print(f"Session created: {session_id}")
        
        # Navigate
        bridge.navigate(session_id, "https://example.com")
        print("Navigated to example.com")
        
        # Get title
        title = bridge.execute(session_id, "return document.title")
        print(f"Title: {title}")
        
        # Close
        bridge.close(session_id)
        print("Session closed")
    else:
        print("❌ WebView Bridge is not running!")
