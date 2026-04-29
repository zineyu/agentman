# Agentman Deployment Guide

This document provides comprehensive deployment instructions for the Agentman Daemon.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Step-by-Step Deployment](#step-by-step-deployment)
- [Configuration Reference](#configuration-reference)
- [Running as systemd Service](#running-as-systemd-service)
- [Multi-Daemon Setup](#multi-daemon-setup)
- [Troubleshooting](#troubleshooting)

---

## Prerequisites

### Required Software

| Component | Minimum Version | Purpose |
|-----------|----------------|---------|
| **Rust** | 1.75+ | Build the daemon |
| **lark-cli** | Latest | Lark Base interaction and auth |
| **Agent CLI** | Latest | At least one of: claude, codex, opencode, cursor |

### Lark Base Requirements

- A Lark (Feishu) application with `bitable:records` scope
- App ID and App Secret from the Lark Developer Console
- Access to the Agentman Base (Token: `YOUR_BASE_TOKEN_HERE`)

### System Requirements

- **OS**: Linux (tested), macOS, Windows
- **RAM**: 512MB minimum, 2GB recommended
- **Disk**: 1GB for binary + workspace directory
- **Network**: Outbound HTTPS to `open.feishu.cn`

---

## Step-by-Step Deployment

### 1. Install Rust

```bash
# Using rustup (recommended)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Verify
rustc --version  # Should be >= 1.75.0
```

### 2. Install Agent CLI Tools

Install at least one supported Agent CLI:

```bash
# Claude Code
npm install -g @anthropic-ai/claude-code
# or
pip install claude-code

# GitHub Copilot CLI (codex)
npm install -g @github/copilot-cli

# OpenCode
npm install -g @opencode/cli

# Cursor
# Download from https://cursor.sh and ensure `cursor` is in PATH
```

Verify installation:
```bash
which claude || echo "Claude not found"
which codex || echo "Codex not found"
which opencode || echo "OpenCode not found"
which cursor || echo "Cursor not found"
```

### 3. Clone Agentman Repository

```bash
# Download the project source code and navigate to the daemon directory
cd agentman/agentman-daemon
```

### 4. Configure the Daemon

Create `config.toml` in the daemon directory:

```toml
# Daemon identity (optional - auto-generated from hostname if omitted)
runtime_id = "agentman-prod-01"
runtime_name = "Production Daemon #1"

# Lark OpenAPI endpoints
base_url = "https://open.feishu.cn"
base_token = "YOUR_BASE_TOKEN_HERE"

# Lark App credentials (from Developer Console)
app_id = "cli_xxxxxxxxxxxxxxxx"
app_secret = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"

# Polling and heartbeat intervals (seconds)
poll_interval_secs = 30
heartbeat_interval_secs = 60

# Concurrency control
max_concurrent_tasks = 3

# Workspace directory for task outputs
workspace_dir = "./workspace"

# Logging level: trace, debug, info, warn, error
log_level = "info"
```

**Important**: Keep `config.toml` secure — it contains sensitive credentials. Add it to `.gitignore`.

### 5. Build the Daemon

```bash
# Release build (recommended for production)
cargo build --release

# The binary will be at:
# ./target/release/agentman-daemon
```

### 6. Register the Runtime

First-time setup automatically registers the daemon in the Lark Base Runtimes table. The daemon will:

1. Check if a runtime with the same hostname already exists
2. If found, reuse the existing `runtime_id` and `runtime_name`
3. If not found, create a new runtime record

```bash
# Register this runtime (or auto-detect existing)
./target/release/agentman-daemon --register

# Expected output:
# INFO agentman_daemon: Agentman Daemon starting...
# INFO agentman_daemon: Runtime ID: agentman-myhostname (auto-detected or configured)
# INFO agentman_daemon: Base URL: https://open.feishu.cn
# INFO agentman_daemon: Registering runtime...
# INFO agentman_daemon::client::base: Reusing existing runtime agentman-myhostname
# ...
# INFO agentman_daemon: Agentman Daemon shutting down...
```

After registration, verify in Lark Base → Runtimes table that your daemon appears with status "在线".

### 7. Run the Daemon

```bash
# Continuous mode (default)
./target/release/agentman-daemon

# Single execution mode (useful for cron or testing)
./target/release/agentman-daemon --once

# With custom config path
./target/release/agentman-daemon --config /etc/agentman/config.toml

# Register + run
./target/release/agentman-daemon --register
```

### 8. Verify Operation

Check logs for successful operation:

```bash
# In another terminal, watch logs
tail -f workspace/logs/agentman.log
```

Expected log patterns:
```
INFO  Fetching pending tasks for runtime agentman-myhostname
INFO  Found 2 pending tasks
INFO  Processing task 42: Implement user authentication
INFO  Workspace directory: ./workspace/task_42
INFO  Task 42 completed successfully, status set to 待审核
INFO  Updated heartbeat for runtime agentman-myhostname
```

---

## Configuration Reference

### `config.toml` Fields

| Field | Type | Default | Required | Description |
|-------|------|---------|----------|-------------|
| `runtime_id` | string | `agentman-<hostname>` | No | Unique daemon identifier (auto-generated from hostname if omitted) |
| `runtime_name` | string | `"Agentman Daemon"` | Yes | Human-readable name shown in Base |
| `base_url` | string | `"https://open.feishu.cn"` | Yes | Lark OpenAPI base URL |
| `base_token` | string | `""` | **Yes** | Lark Base app token |
| `app_id` | string | `""` | **Yes** | Lark application ID |
| `app_secret` | string | `""` | **Yes** | Lark application secret |
| `poll_interval_secs` | u64 | `30` | No | Task polling interval (seconds) |
| `heartbeat_interval_secs` | u64 | `60` | No | Heartbeat interval (seconds) |
| `max_concurrent_tasks` | usize | `3` | No | Max concurrent task executions |
| `workspace_dir` | string | `"./workspace"` | No | Directory for task workspaces and outputs |
| `log_level` | string | `"info"` | No | Log verbosity level |

### Environment Variables

Agentman does not currently use environment variables for configuration. All settings are read from `config.toml`.

### Log Levels

| Level | Description |
|-------|-------------|
| `trace` | Extremely verbose, includes API request/response bodies |
| `debug` | API calls, token refresh, heartbeat details |
| `info` | Task processing, status changes, registration |
| `warn` | Retryable errors, rate limiting, timeouts |
| `error` | Fatal errors, task failures, auth failures |

---

## Running as systemd Service

### Quick Install (Recommended)

Use the provided install script for one-command deployment:

```bash
cd agentman-daemon
./install.sh                    # Install to /opt/agentman (default)
# or
./install.sh /usr/local/agentman # Custom install directory
```

The script will:
1. Build the release binary
2. Create a dedicated `agentman` user
3. Install files to the target directory
4. Install and enable the systemd service

### Manual Service Setup

If you prefer manual setup, create `/etc/systemd/system/agentman-daemon.service`:

```ini
[Unit]
Description=Agentman Task Management Daemon
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=agentman
Group=agentman
WorkingDirectory=/opt/agentman
ExecStart=/opt/agentman/agentman-daemon
Restart=on-failure
RestartSec=10
Environment="RUST_LOG=info"

# Security
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/agentman/workspace /opt/agentman/logs
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true

# Resource limits
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

### Setup Service User

```bash
# Create dedicated user
sudo useradd -r -s /bin/false -d /opt/agentman agentman

# Create directories
sudo mkdir -p /opt/agentman/workspace
sudo cp target/release/agentman-daemon /opt/agentman/
sudo cp config.toml /opt/agentman/

# Set permissions
sudo chown -R agentman:agentman /opt/agentman
sudo chmod 600 /opt/agentman/config.toml
```

### Manage Service

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable auto-start
sudo systemctl enable agentman-daemon

# Start service
sudo systemctl start agentman-daemon

# Check status
sudo systemctl status agentman-daemon

# View logs
sudo journalctl -u agentman-daemon -f

# Restart
sudo systemctl restart agentman-daemon

# Stop
sudo systemctl stop agentman-daemon
```

### Log Rotation

Create `/etc/logrotate.d/agentman`:

```
/opt/agentman/workspace/logs/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    create 0644 agentman agentman
}
```

---

## Multi-Daemon Setup

You can run multiple Agentman Daemon instances for high availability or load distribution.

### Approach 1: Per-Machine Daemon

Each machine runs one daemon. `runtime_id` is auto-generated from hostname if not configured:

```toml
# Machine 1: config.toml (auto-detects as agentman-web-server-01)
runtime_name = "Web Server Daemon"

# Machine 2: config.toml (auto-detects as agentman-api-server-01)
runtime_name = "API Server Daemon"

# Machine 3: config.toml (auto-detects as agentman-gpu-worker-01)
runtime_name = "GPU Worker Daemon"
```

To override the auto-detected ID, explicitly set `runtime_id`:
```toml
# Machine 1 with custom ID
runtime_id = "agentman-web-01"
runtime_name = "Web Server Daemon"
```

Tasks are pre-allocated in Lark Base by setting the "分配的运行时" (Assigned Runtime) field to link to a specific runtime record.

### Approach 2: Multiple Daemons on Same Machine

Use different config files and workspace directories. Each daemon **must** have a unique `runtime_id`:

```bash
# Daemon 1
cargo run -- --config daemon1.toml

# Daemon 2 (different terminal)
cargo run -- --config daemon2.toml
```

```toml
# daemon1.toml
runtime_id = "agentman-local-01"
workspace_dir = "./workspace1"
poll_interval_secs = 30

# daemon2.toml
runtime_id = "agentman-local-02"
workspace_dir = "./workspace2"
poll_interval_secs = 45
```

### Load Distribution Strategy

| Strategy | How To | Use Case |
|----------|--------|----------|
| **Static Assignment** | Set "分配的运行时" field in Base | Predictable workload per daemon |
| **Round-Robin** | Alternate assignment manually | Balanced distribution |
| **Capability-Based** | Assign by available agents (e.g., GPU tasks to GPU daemon) | Specialized hardware |
| **Priority-Based** | P0 tasks to fastest daemon, P3 to slower | SLA requirements |

---

## Troubleshooting

### Common Issues

#### 1. Token Refresh Failed

**Symptoms**:
```
WARN Failed to send heartbeat: Token refresh failed: API error 99991663: app ticket invalid
```

**Solutions**:
- Verify `app_id` and `app_secret` are correct
- Ensure the Lark app is published and has `bitable:records` scope
- Check if the app is in "开发中" mode — it may need to be published or use test mode

#### 2. No CLI Tool Found

**Symptoms**:
```
ERROR Failed to create agent adapter: No CLI tool found for ClaudeCode. Tried: ["claude", "claude-code"]
```

**Solutions**:
- Install the required Agent CLI: `npm install -g @anthropic-ai/claude-code`
- Verify it's in PATH: `which claude`
- For custom install locations, create a symlink: `ln -s /custom/path/claude /usr/local/bin/claude`

#### 3. Rate Limiting (429)

**Symptoms**:
```
WARN HTTP 429 rate limited, retrying in 2s
WARN Retryable API error 1254290, retrying in 4s
```

**Solutions**:
- This is usually handled automatically by the retry logic
- If persistent, increase `poll_interval_secs` to 60 or higher
- Check if multiple daemons are hitting the same Base app

#### 5. Workspace Permission Denied

**Symptoms**:
```
ERROR Failed to create workspace directory: Permission denied
```

**Solutions**:
- Ensure the daemon user owns the workspace directory: `sudo chown -R agentman:agentman /opt/agentman/workspace`
- Check SELinux/AppArmor if on hardened systems
- Verify `ProtectSystem=strict` isn't blocking writes (systemd)

#### 6. Task Stuck in "进行中"

**Symptoms**: Task status stays "进行中" indefinitely.

**Solutions**:
- Check if the Agent CLI is still running: `ps aux | grep claude`
- The default timeout is 30 minutes — wait or kill the process
- Check execution logs in Lark Base ExecutionLogs table

### Debug Mode

Run with debug logging for detailed diagnostics:

```bash
RUST_LOG=debug ./target/release/agentman-daemon --once
```

This shows:
- Every API request/response
- Token refresh events
- Agent CLI spawn details

### Health Check Script

```bash
#!/bin/bash
# /opt/agentman/healthcheck.sh

PID=$(pgrep -f agentman-daemon)
if [ -z "$PID" ]; then
    echo "FAIL: Daemon not running"
    exit 1
fi

# Check if heartbeat updated in last 2 minutes
# (Customize based on your heartbeat_interval)
LAST_HEARTBEAT=$(journalctl -u agentman-daemon --since "2 minutes ago" | grep "Heartbeat sent" | tail -1)
if [ -z "$LAST_HEARTBEAT" ]; then
    echo "WARN: No recent heartbeat"
    exit 1
fi

echo "OK: Daemon healthy (PID: $PID)"
exit 0
```

### Getting Help

1. Check logs: `journalctl -u agentman-daemon -n 100`
2. Run with backtrace: `RUST_BACKTRACE=1 cargo run`
3. Verify Base table structure matches expected schema
4. Ensure Lark app has required scopes: `bitable:records`, `bitable:record`

---

## Security Considerations

- **Config file**: Set permissions to `600` (owner read/write only)
- **App Secret**: Rotate regularly in Lark Developer Console
- **Workspace**: Clean up old workspaces to prevent disk exhaustion
- **Network**: Daemon only needs outbound HTTPS; no inbound ports required
- **User isolation**: Run daemon as non-root, dedicated user
