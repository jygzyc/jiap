# 🚀 JIAP v0.2.0 Release Notes

## ✨ Features
- **Plugin Architecture Refactor**: Introduced JiapConfig with centralized service management and routing
- **Enhanced Cross-Reference Analysis**: Improved method/class cross-references with better line number tracking
- **UI Integration**: Added selected text/class endpoints for real-time JADX GUI interaction
- **Android System Service Analysis**: Enhanced system service implementation discovery

## ⚡ Improvements
- **Server-Side Caching**: Moved caching logic from client to core server for better performance
- **Simplified MCP Server**: Removed client-side slicing and complex cache management
- **Enhanced Error Handling**: Better exception management with structured error responses
- **Code Quality**: Cleaned up duplicated code, improved type safety and service organization

---

**Breaking Changes**: Removed client-side caching, simplified MCP response format
**Migration**: Update MCP clients for new response structure