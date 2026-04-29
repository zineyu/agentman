#!/bin/bash
set -euo pipefail

# Agentman Daemon Installation Script
# Usage: ./install.sh [install_dir]

INSTALL_DIR="${1:-/opt/agentman}"
SERVICE_NAME="agentman-daemon"
USER_NAME="agentman"

print_info() { echo "[INFO] $1"; }
print_warn() { echo "[WARN] $1"; }
print_error() { echo "[ERROR] $1" >&2; }

check_prerequisites() {
    print_info "Checking prerequisites..."

    if ! command -v cargo &>/dev/null; then
        print_error "Rust/Cargo not found. Install from https://rustup.rs"
        exit 1
    fi

    local rust_version
    rust_version=$(rustc --version | awk '{print $2}')
    print_info "Rust version: $rust_version"
}

build_release() {
    print_info "Building release binary..."
    cargo build --release
    print_info "Build complete: $(ls -lh target/release/agentman-daemon | awk '{print $5}')"
}

setup_user() {
    if id "$USER_NAME" &>/dev/null; then
        print_warn "User '$USER_NAME' already exists"
    else
        print_info "Creating user: $USER_NAME"
        sudo useradd -r -s /usr/sbin/nologin -d "$INSTALL_DIR" "$USER_NAME"
    fi
}

install_files() {
    print_info "Installing to $INSTALL_DIR..."
    sudo mkdir -p "$INSTALL_DIR/workspace"
    sudo mkdir -p "$INSTALL_DIR/logs"

    sudo cp target/release/agentman-daemon "$INSTALL_DIR/"
    sudo chmod +x "$INSTALL_DIR/agentman-daemon"

    if [[ -f config.toml ]]; then
        sudo cp config.toml "$INSTALL_DIR/"
        sudo chmod 600 "$INSTALL_DIR/config.toml"
    else
        print_warn "config.toml not found. Copy manually to $INSTALL_DIR/"
    fi

    sudo chown -R "$USER_NAME:$USER_NAME" "$INSTALL_DIR"
}

install_systemd() {
    print_info "Installing systemd service..."

    local service_file="/etc/systemd/system/${SERVICE_NAME}.service"

    sudo tee "$service_file" > /dev/null <<EOF
[Unit]
Description=Agentman Task Management Daemon
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$USER_NAME
Group=$USER_NAME
WorkingDirectory=$INSTALL_DIR
ExecStart=$INSTALL_DIR/agentman-daemon
Restart=on-failure
RestartSec=10
Environment="RUST_LOG=info"

# Security
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=$INSTALL_DIR/workspace $INSTALL_DIR/logs
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true

# Resource limits
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

    sudo systemctl daemon-reload
    sudo systemctl enable "$SERVICE_NAME"
    print_info "Systemd service installed. Start with: sudo systemctl start $SERVICE_NAME"
}

print_next_steps() {
    echo ""
    echo "========================================"
    echo "  Agentman Daemon Installation Complete"
    echo "========================================"
    echo ""
    echo "Install directory: $INSTALL_DIR"
    echo ""
    echo "Next steps:"
    echo "  1. Edit config:   sudo nano $INSTALL_DIR/config.toml"
    echo "  2. Register:      sudo $INSTALL_DIR/agentman-daemon --register"
    echo "  3. Start daemon:  sudo systemctl start $SERVICE_NAME"
    echo "  4. View logs:     sudo journalctl -u $SERVICE_NAME -f"
    echo "  5. Check status:  sudo systemctl status $SERVICE_NAME"
    echo ""
}

main() {
    print_info "Agentman Daemon Installer"
    print_info "Target directory: $INSTALL_DIR"

    check_prerequisites
    build_release
    setup_user
    install_files
    install_systemd
    print_next_steps
}

main "$@"
