#!/usr/bin/env bash
set -euo pipefail

REPO="${HELIX_REPO:-TheDarkLightX/Helix-A-Next-Gen-Event-Driven-Multi-Agent-Platform}"
VERSION="${HELIX_VERSION:-latest}"
BIN_DIR="${HELIX_INSTALL_BIN_DIR:-$HOME/.local/bin}"
DATA_DIR="${HELIX_INSTALL_DATA_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/helix}"

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "[install] missing required command: $cmd" >&2
    exit 1
  fi
}

case "$(uname -s)" in
  Linux) os="linux" ;;
  Darwin) os="darwin" ;;
  *) echo "[install] unsupported OS: $(uname -s)" >&2; exit 1 ;;
esac

case "$(uname -m)" in
  x86_64 | amd64) arch="x64" ;;
  arm64 | aarch64) arch="arm64" ;;
  *) echo "[install] unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

require_cmd curl
require_cmd tar
require_cmd install

asset="helix-api-${os}-${arch}.tar.gz"
if [[ "$VERSION" == "latest" ]]; then
  url="https://github.com/${REPO}/releases/latest/download/${asset}"
else
  url="https://github.com/${REPO}/releases/download/${VERSION}/${asset}"
fi

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

echo "[install] downloading $url"
curl -fsSL "$url" -o "$tmp_dir/$asset"

echo "[install] unpacking $asset"
tar -xzf "$tmp_dir/$asset" -C "$tmp_dir"
package_dir="$(find "$tmp_dir" -maxdepth 1 -type d -name 'helix-api-*' | head -n 1)"
if [[ -z "$package_dir" ]]; then
  echo "[install] archive did not contain a helix-api package directory" >&2
  exit 1
fi

mkdir -p "$BIN_DIR" "$DATA_DIR"
cp -R "$package_dir/." "$DATA_DIR/"
cat >"$BIN_DIR/helix" <<EOF
#!/usr/bin/env bash
set -euo pipefail
if [[ -z "\${HELIX_UI_DIST:-}" ]]; then
  export HELIX_UI_DIST="$DATA_DIR/ui/dist"
fi
exec "$DATA_DIR/bin/helix-api" "\$@"
EOF
cp "$BIN_DIR/helix" "$BIN_DIR/helix-api"
chmod 0755 "$BIN_DIR/helix"
chmod 0755 "$BIN_DIR/helix-api"

echo "[install] installed helix launcher to $BIN_DIR/helix"
echo "[install] installed helix-api compatibility launcher to $BIN_DIR/helix-api"
echo "[install] release payload copied to $DATA_DIR"
echo "[install] run: helix"
