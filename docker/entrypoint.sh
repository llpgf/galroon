#!/bin/sh
# Galroon Docker runtime launcher with atomic self-update and automatic rollback.
# Runs as PID 1 (non-root) inside the container, forwards SIGTERM/SIGINT to Core,
# restarts on exit code 42 after a verified self-update, and rolls back to the
# previous healthy release if a newly applied version exits before becoming healthy.
set -eu

BUILTIN_BIN="/opt/galroon/galroon-core"
RUNTIME_DIR="${GALROON_RUNTIME_DIR:-/data/runtime}"
UPDATE_EXIT_CODE=42

# Non-serve invocations (e.g. `docker exec galroon /opt/galroon/entrypoint.sh owner ...`)
# delegate directly to the currently active binary without entering the supervision loop.
if [ "${1:-}" != "serve" ]; then
    ACTIVE_BIN="$BUILTIN_BIN"
    if [ -f "$RUNTIME_DIR/current" ]; then
        CUR_VER=$(tr -d '\r\n' < "$RUNTIME_DIR/current" 2>/dev/null || true)
        case "$CUR_VER" in
            *[!/0-9A-Za-z._-]*|""|*/*|*..*) ;;
            *)
                if [ -x "$RUNTIME_DIR/releases/$CUR_VER/galroon-core" ]; then
                    ACTIVE_BIN="$RUNTIME_DIR/releases/$CUR_VER/galroon-core"
                fi
                ;;
        esac
    fi
    exec "$ACTIVE_BIN" "$@"
fi

mkdir -p "$RUNTIME_DIR/releases" 2>/dev/null || true

# If the container image itself was manually upgraded to a newer version than
# /data/runtime/current (for example via deploy.sh), clear stale runtime pointers
# so the newly deployed container image takes effect immediately.
BUILTIN_VER=$("$BUILTIN_BIN" --version 2>/dev/null | tr -d '\r\n' || true)
if [ -n "$BUILTIN_VER" ] && [ -f "$RUNTIME_DIR/current" ]; then
    CUR_VER=$(tr -d '\r\n' < "$RUNTIME_DIR/current" 2>/dev/null || true)
    if [ -n "$CUR_VER" ] && command -v dpkg >/dev/null 2>&1; then
        if dpkg --compare-versions "$BUILTIN_VER" gt "$CUR_VER" 2>/dev/null; then
            rm -f "$RUNTIME_DIR/current" "$RUNTIME_DIR/pending" "$RUNTIME_DIR/attempt" "$RUNTIME_DIR/healthy"
        fi
    fi
fi

STOP_REQUESTED=0
CHILD_PID=""

forward_signal() {
    STOP_REQUESTED=1
    if [ -n "$CHILD_PID" ]; then
        kill "-$1" "$CHILD_PID" 2>/dev/null || true
    fi
}

trap 'forward_signal TERM' TERM
trap 'forward_signal INT' INT

valid_version() {
    case "$1" in
        ""|*/*|*..*|*[!0-9A-Za-z._-]*) return 1 ;;
        *) return 0 ;;
    esac
}

record_rollback() {
    ver="$1"
    code="$2"
    printf '%s\nUpdate to %s failed during startup (exit %s) and rolled back automatically.\n' "$ver" "$ver" "$code" > "$RUNTIME_DIR/last-failed.tmp" 2>/dev/null \
        && mv -f "$RUNTIME_DIR/last-failed.tmp" "$RUNTIME_DIR/last-failed" 2>/dev/null || true
}

while true; do
    TRYING_PENDING=""
    BIN="$BUILTIN_BIN"

    # If a previous attempt crashed the container before Core wrote healthy, roll it back.
    if [ -f "$RUNTIME_DIR/attempt" ]; then
        STALE_ATTEMPT=$(tr -d '\r\n' < "$RUNTIME_DIR/attempt" 2>/dev/null || true)
        HEALTHY_VER=""
        if [ -f "$RUNTIME_DIR/healthy" ]; then
            HEALTHY_VER=$(tr -d '\r\n' < "$RUNTIME_DIR/healthy" 2>/dev/null || true)
        fi
        if [ -n "$STALE_ATTEMPT" ] && [ "$STALE_ATTEMPT" != "$HEALTHY_VER" ]; then
            echo "galroon-launcher: previous update attempt $STALE_ATTEMPT did not reach healthy state; rolling back" >&2
            record_rollback "$STALE_ATTEMPT" "crash"
            if valid_version "$STALE_ATTEMPT"; then
                rm -rf "$RUNTIME_DIR/releases/$STALE_ATTEMPT" 2>/dev/null || true
            fi
        fi
        rm -f "$RUNTIME_DIR/attempt" 2>/dev/null || true
    fi

    if [ -f "$RUNTIME_DIR/pending" ]; then
        PENDING_VER=$(tr -d '\r\n' < "$RUNTIME_DIR/pending" 2>/dev/null || true)
        if valid_version "$PENDING_VER" && [ -x "$RUNTIME_DIR/releases/$PENDING_VER/galroon-core" ]; then
            rm -f "$RUNTIME_DIR/healthy" 2>/dev/null || true
            mv -f "$RUNTIME_DIR/pending" "$RUNTIME_DIR/attempt"
            TRYING_PENDING="$PENDING_VER"
            BIN="$RUNTIME_DIR/releases/$PENDING_VER/galroon-core"
            echo "galroon-launcher: starting staged update $PENDING_VER" >&2
        else
            rm -f "$RUNTIME_DIR/pending" 2>/dev/null || true
        fi
    fi

    if [ -z "$TRYING_PENDING" ] && [ -f "$RUNTIME_DIR/current" ]; then
        CUR_VER=$(tr -d '\r\n' < "$RUNTIME_DIR/current" 2>/dev/null || true)
        if valid_version "$CUR_VER" && [ -x "$RUNTIME_DIR/releases/$CUR_VER/galroon-core" ]; then
            BIN="$RUNTIME_DIR/releases/$CUR_VER/galroon-core"
        else
            rm -f "$RUNTIME_DIR/current" 2>/dev/null || true
        fi
    fi

    export GALROON_LAUNCHER=1
    export GALROON_RUNTIME_DIR="$RUNTIME_DIR"
    export GALROON_WEB_DIR="$(dirname "$BIN")/web"

    "$BIN" "$@" &
    CHILD_PID=$!

    EXIT_CODE=0
    while true; do
        set +e
        wait "$CHILD_PID"
        EXIT_CODE=$?
        set -e
        if kill -0 "$CHILD_PID" 2>/dev/null; then
            continue
        fi
        break
    done
    CHILD_PID=""

    if [ "$STOP_REQUESTED" = "1" ]; then
        exit "$EXIT_CODE"
    fi

    if [ -n "$TRYING_PENDING" ]; then
        HEALTHY_VER=""
        if [ -f "$RUNTIME_DIR/healthy" ]; then
            HEALTHY_VER=$(tr -d '\r\n' < "$RUNTIME_DIR/healthy" 2>/dev/null || true)
        fi
        if [ "$HEALTHY_VER" != "$TRYING_PENDING" ]; then
            echo "galroon-launcher: update $TRYING_PENDING exited ($EXIT_CODE) before becoming healthy; rolling back" >&2
            record_rollback "$TRYING_PENDING" "$EXIT_CODE"
            rm -f "$RUNTIME_DIR/attempt" "$RUNTIME_DIR/healthy" 2>/dev/null || true
            rm -rf "$RUNTIME_DIR/releases/$TRYING_PENDING" 2>/dev/null || true
            continue
        fi
        rm -f "$RUNTIME_DIR/attempt" 2>/dev/null || true
    fi

    if [ "$EXIT_CODE" = "$UPDATE_EXIT_CODE" ]; then
        echo "galroon-launcher: graceful restart requested for update" >&2
        continue
    fi

    exit "$EXIT_CODE"
done
