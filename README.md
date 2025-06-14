# JIAP - Java Intelligence Analysis Platform

An AI-powered Java application analysis platform built on JADX decompiler, providing comprehensive code analysis capabilities through Model Context Protocol (MCP) for AI assistants.

## Overview

JIAP is a modular platform that combines the power of JADX decompilation with AI-driven analysis capabilities. It consists of three core modules: file preprocessing, JADX server, and MCP server, designed to provide seamless Java code analysis for AI assistants and developers.

**Warning**: Please note that decompilation may not be 100% accurate in all cases, and errors may occur during the analysis process.

## Main Features

- **Comprehensive Decompilation**: Support for APK, DEX, JAR, APEX, and other Android file formats 
- **AI Integration**: Native MCP support for seamless AI assistant integration
- **Advanced Analysis**: Cross-reference analysis, inheritance relationships, and system service mapping
- **RESTful API**: Complete HTTP API for programmatic access
- **File Preprocessing**: Automated extraction and processing of Android framework files
- **Multi-format Support**: Java and Smali source code output

## Architecture

```
├── processor_tools/
│   └── file_preprocessor/     # File preprocessing module
├── jadx_server/              # JADX decompilation server
└── mcp_server/              # MCP protocol server
```

### Core Modules

#### 1. File Preprocessor
- **Purpose**: Extract and preprocess Android framework files from devices
- **Supported Formats**: JAR, APEX, DEX files
- **Key Features**:
  - ADB automatic file synchronization
  - APEX file extraction and processing
  - DEX to JAR conversion
  - Multi-threaded parallel processing

#### 2. JADX Server
- **Technology Stack**: Spring Boot + Kotlin
- **Purpose**: Provides RESTful API endpoints for code decompilation and analysis
- **Core Features**:
  - Class and method search capabilities
  - Source code retrieval (Java/Smali)
  - Cross-reference analysis
  - Inheritance relationship mapping
  - Android system service analysis

#### 3. JADX MCP Server
- **Technology Stack**: Python + FastMCP
- **Purpose**: Provides MCP protocol interface for AI assistants
- **Features**: Asynchronous HTTP client, error handling, tool function encapsulation

## Requirements

- Java 11 or later (64-bit version recommended)
- Python 3.8+
- Android SDK (optional, for device file extraction)

## Installation

### Build from Source

```bash
git clone <repository-url>
cd android_ai_analysis
```

### 1. Start JADX Server

```bash
cd jadx_server
./gradlew bootRun
# Or use the provided script
./run_jadx_server.sh
```

### 2. Start MCP Server

```bash
cd jadx_mcp_server
python jadx_mcp_server.py [JADX_SERVER_URI]
# Default connection: http://127.0.0.1:8080/api/jadx
```

### 3. File Preprocessing (Optional)

```bash
cd processor_tools/file_preprocessor
python fetch_file_from_device.py  # Extract files from device
python extract_framework_code.py  # Process framework files
```

## Usage

### RESTful API Endpoints

The JADX server provides comprehensive REST API endpoints for Android code analysis:

| Endpoint | Method | Description | Parameters |
|----------|--------|-------------|------------|
| `/api/jadx/get_all_classes` | POST | Get all class list | None |
| `/api/jadx/search_class_by_name` | POST | Search class by name | `{"class": "className"}` |
| `/api/jadx/search_method_by_name` | POST | Search methods by name | `{"method": "methodName"}` |
| `/api/jadx/list_methods_of_class` | POST | List all methods of a class | `{"class": "className"}` |
| `/api/jadx/list_fields_of_class` | POST | List all fields of a class | `{"class": "className"}` |
| `/api/jadx/get_class_source` | POST | Get class source code | `{"class": "className", "smali": false}` |
| `/api/jadx/get_method_source` | POST | Get method source code | `{"method": "methodInfo", "smali": false}` |
| `/api/jadx/get_method_xref` | POST | Get method cross-references | `{"method": "methodInfo"}` |
| `/api/jadx/get_class_xref` | POST | Get class cross-references | `{"class": "className"}` |
| `/api/jadx/get_field_xref` | POST | Get field cross-references | `{"field": "fieldInfo"}` |
| `/api/jadx/get_interface_impl` | POST | Get interface implementations | `{"interface": "interfaceName"}` |
| `/api/jadx/get_subclass` | POST | Get subclasses | `{"class": "className"}` |
| `/api/jadx/get_system_service_impl` | POST | Get system service implementations | `{"class": "serviceName"}` |
| `/api/jadx/test` | POST | Test endpoint | None |

### MCP Integration

The MCP server provides seamless integration with AI assistants, offering the same functionality through the Model Context Protocol. Refer to the [MCP Server Documentation](jadx_mcp_server/README.md) for detailed usage instructions.

### File Upload

The platform supports file upload through the web interface at `http://localhost:8080` when the JADX server is running. Supported file formats include APK, DEX, JAR, and other Android-related files.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [JADX](https://github.com/skylot/jadx) - The core decompilation engine <mcreference link="https://github.com/skylot/jadx/blob/master/README.md" index="5">5</mcreference>
- [Spring Boot](https://spring.io/projects/spring-boot) - Web framework for the server component
- [Model Context Protocol](https://modelcontextprotocol.io/) - AI assistant integration protocol