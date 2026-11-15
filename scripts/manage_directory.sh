#!/usr/bin/env bash
# ==============================================================================
# BlockGame Central Directory Server Manager (Non-systemd)
# ==============================================================================
# Easily build, run, monitor, and manage the central server directory on Ubuntu
# without creating a systemd service.
#
# Usage:
#   ./scripts/manage_directory.sh start    # Start in background with auto-restart loop
#   ./scripts/manage_directory.sh stop     # Stop background process
#   ./scripts/manage_directory.sh restart  # Restart background process
#   ./scripts/manage_directory.sh status   # Show running status & health check
#   ./scripts/manage_directory.sh logs     # Follow live log output
#   ./scripts/manage_directory.sh tmux     # Launch inside a detachable tmux session
#   ./scripts/manage_directory.sh screen   # Launch inside a detachable screen session
#   ./scripts/manage_directory.sh build    # Compile target/release/blockgame_directory
#
# Environment variables (optional):
#   DIRECTORY_PORT=25569
#   DIRECTORY_BIND=0.0.0.0
#   RUST_LOG=info
# ==============================================================================

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

PORT="${DIRECTORY_PORT:-25569}"
BIND="${DIRECTORY_BIND:-0.0.0.0}"
export RUST_LOG="${RUST_LOG:-info}"

BIN_PATH="$ROOT_DIR/target/release/blockgame_directory"
PID_FILE="$ROOT_DIR/.directory_server.pid"
LOG_FILE="$ROOT_DIR/directory_server.log"
SESSION_NAME="blockgame-directory"

build_binary() {
    echo "==> Building blockgame_directory (release mode)..."
    cd "$ROOT_DIR"
    cargo build --release -p blockgame_directory
    echo "==> Build complete: $BIN_PATH"
}

is_running() {
    if [[ -f "$PID_FILE" ]]; then
        local pid
        pid=$(cat "$PID_FILE" 2>/dev/null || true)
        if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
            return 0
        fi
    fi
    # Fallback: check if the process binary is running
    if pgrep -f "$BIN_PATH" >/dev/null 2>&1; then
        return 0
    fi
    return 1
}

cmd_start() {
    if is_running; then
        local pid
        pid=$(cat "$PID_FILE" 2>/dev/null || pgrep -f "$BIN_PATH" | head -n1)
        echo "Directory server is ALREADY running (PID: ${pid:-unknown})."
        echo "Check status with: $0 status"
        return 0
    fi

    if [[ ! -x "$BIN_PATH" ]]; then
        echo "Binary not found at $BIN_PATH."
        build_binary
    fi

    echo "==> Starting BlockGame Central Directory server on ${BIND}:${PORT}..."

    # Launch watchdog loop detached from terminal
    nohup bash -c '
        BIN="'"$BIN_PATH"'"
        P="'"$PORT"'"
        B="'"$BIND"'"
        while true; do
            echo "[$(date "+%Y-%m-%d %H:%M:%S")] Starting central directory on ${B}:${P}..."
            "$BIN" --bind "$B" --port "$P"
            EXIT_CODE=$?
            echo "[$(date "+%Y-%m-%d %H:%M:%S")] Directory server exited ($EXIT_CODE). Restarting in 2s..."
            sleep 2
        done
    ' < /dev/null >> "$LOG_FILE" 2>&1 &

    local runner_pid=$!
    echo "$runner_pid" > "$PID_FILE"
    disown "$runner_pid" 2>/dev/null || true

    echo "==> Started watchdog runner (PID: $runner_pid)."
    echo "    Log file: $LOG_FILE"
    echo "    Port:     $PORT"

    sleep 1
    if is_running; then
        echo "==> Server successfully running."
        if command -v curl >/dev/null 2>&1; then
            local health
            health=$(curl -s "http://127.0.0.1:${PORT}/health" 2>/dev/null || true)
            if [[ -n "$health" ]]; then
                echo "    Health response: $health"
            fi
        fi
    else
        echo "==> Warning: Process did not stay alive. Check logs:"
        tail -n 20 "$LOG_FILE"
        exit 1
    fi
}

cmd_stop() {
    local stopped=false
    if [[ -f "$PID_FILE" ]]; then
        local pid
        pid=$(cat "$PID_FILE" 2>/dev/null || true)
        if [[ -n "$pid" ]]; then
            echo "==> Stopping watchdog process group (PID: $pid)..."
            pkill -P "$pid" 2>/dev/null || true
            kill "$pid" 2>/dev/null || true
            stopped=true
        fi
        rm -f "$PID_FILE"
    fi

    if pgrep -f "$BIN_PATH" >/dev/null 2>&1; then
        echo "==> Terminating blockgame_directory instances..."
        pkill -f "$BIN_PATH" 2>/dev/null || true
        sleep 1
        # Force kill if still lingering
        pkill -9 -f "$BIN_PATH" 2>/dev/null || true
        stopped=true
    fi

    if [[ "$stopped" = true ]]; then
        echo "==> Central Directory Server stopped."
    else
        echo "Directory server was not running."
    fi
}

cmd_status() {
    if is_running; then
        local pid
        pid=$(cat "$PID_FILE" 2>/dev/null || pgrep -f "$BIN_PATH" | head -n1)
        echo "Status: RUNNING (PID: $pid)"
        echo "Port:   $PORT (bind: $BIND)"
        echo "Log:    $LOG_FILE"
        if command -v curl >/dev/null 2>&1; then
            echo -n "Health (GET /health): "
            curl -s "http://127.0.0.1:${PORT}/health" 2>/dev/null || echo "(no response)"
            echo ""
            echo -n "Active Servers (GET /api/servers): "
            curl -s "http://127.0.0.1:${PORT}/api/servers" 2>/dev/null || echo "(no response)"
            echo ""
        fi
    else
        echo "Status: STOPPED"
        if [[ -f "$PID_FILE" ]]; then
            rm -f "$PID_FILE"
        fi
    fi
}

cmd_logs() {
    if [[ ! -f "$LOG_FILE" ]]; then
        touch "$LOG_FILE"
    fi
    echo "Following $LOG_FILE (Press Ctrl+C to exit)..."
    tail -f "$LOG_FILE"
}

cmd_tmux() {
    if ! command -v tmux >/dev/null 2>&1; then
        echo "Error: tmux is not installed. Install with: sudo apt install tmux"
        exit 1
    fi

    if [[ ! -x "$BIN_PATH" ]]; then
        echo "Binary not found at $BIN_PATH."
        build_binary
    fi

    if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
        echo "Tmux session '$SESSION_NAME' is already running."
        echo "Attaching to session (press 'Ctrl+B' then 'D' to detach)..."
        tmux attach -t "$SESSION_NAME"
        return 0
    fi

    echo "==> Launching directory server in tmux session: $SESSION_NAME"
    tmux new-session -d -s "$SESSION_NAME" \
        "bash -c 'while true; do echo \"[$(date \"+%Y-%m-%d %H:%M:%S\")] Starting directory server on ${BIND}:${PORT}...\"; \"$BIN_PATH\" --bind \"$BIND\" --port \"$PORT\"; echo \"Server exited. Restarting in 2s...\"; sleep 2; done'"

    echo "==> Session '$SESSION_NAME' started!"
    echo "    To view/attach:   tmux attach -t $SESSION_NAME"
    echo "    To detach:        Ctrl+B then D"
    echo "    To stop/kill:     tmux kill-session -t $SESSION_NAME"
}

cmd_screen() {
    if ! command -v screen >/dev/null 2>&1; then
        echo "Error: screen is not installed. Install with: sudo apt install screen"
        exit 1
    fi

    if [[ ! -x "$BIN_PATH" ]]; then
        echo "Binary not found at $BIN_PATH."
        build_binary
    fi

    if screen -list | grep -q "\.${SESSION_NAME}[[:space:]]"; then
        echo "Screen session '$SESSION_NAME' is already running."
        echo "Attaching to session (press 'Ctrl+A' then 'D' to detach)..."
        screen -r "$SESSION_NAME"
        return 0
    fi

    echo "==> Launching directory server in screen session: $SESSION_NAME"
    screen -dmS "$SESSION_NAME" \
        bash -c "while true; do echo \"[$(date \"+%Y-%m-%d %H:%M:%S\")] Starting directory server on ${BIND}:${PORT}...\"; \"$BIN_PATH\" --bind \"$BIND\" --port \"$PORT\"; echo \"Server exited. Restarting in 2s...\"; sleep 2; done"

    echo "==> Screen session '$SESSION_NAME' started!"
    echo "    To view/attach:   screen -r $SESSION_NAME"
    echo "    To detach:        Ctrl+A then D"
    echo "    To stop/kill:     screen -X -S $SESSION_NAME quit"
}

case "${1:-}" in
    start)
        cmd_start
        ;;
    stop)
        cmd_stop
        ;;
    restart)
        cmd_stop
        sleep 1
        cmd_start
        ;;
    status)
        cmd_status
        ;;
    logs)
        cmd_logs
        ;;
    tmux)
        cmd_tmux
        ;;
    screen)
        cmd_screen
        ;;
    build)
        build_binary
        ;;
    *)
        echo "Usage: $0 {start|stop|restart|status|logs|tmux|screen|build}"
        echo ""
        echo "Commands:"
        echo "  start    - Start in background with auto-restart & logging"
        echo "  stop     - Stop background server and watchdog"
        echo "  restart  - Restart background server"
        echo "  status   - Check process state and HTTP endpoints"
        echo "  logs     - Stream live logs ($LOG_FILE)"
        echo "  tmux     - Run inside an interactive/detachable tmux session"
        echo "  screen   - Run inside an interactive/detachable screen session"
        echo "  build    - Compile target/release/blockgame_directory"
        exit 1
        ;;
esac
