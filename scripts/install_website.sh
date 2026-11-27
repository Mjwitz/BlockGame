#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# Roots & Ridges (rootsandridges.gg) — One-Command Server Installer
# Supports:
#   1) Docker Compose (recommended if Docker is installed)
#   2) Native Caddy systemd service (lightweight, zero Docker required)
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
WEBSITE_SRC="${REPO_ROOT}/website"

DOMAIN="${DOMAIN:-rootsandridges.gg}"
MODE="${MODE:-auto}"       # auto | docker | native
NO_TLS="${NO_TLS:-false}"  # true = serve plain HTTP on HTTP_PORT (for Cloudflare/Nginx proxy)
HTTP_PORT="${HTTP_PORT:-80}"
HTTPS_PORT="${HTTPS_PORT:-443}"
INSTALL_DIR="/var/www/rootsandridges"

usage() {
  cat <<EOF
Usage: sudo $0 [options]

Options:
  --mode <docker|native|auto>   Install method (default: auto — uses Docker if available, else native Caddy)
  --domain <domain>             Primary domain (default: rootsandridges.gg)
  --no-tls                      Disable automatic HTTPS (use if behind Nginx/Cloudflare Tunnel)
  --http-port <port>            HTTP port to bind (default: 80, or 8080 if --no-tls)
  -h, --help                    Show this help message

Examples:
  # Standard production server with automatic Let's Encrypt HTTPS:
  sudo $0

  # Behind an existing reverse proxy (Nginx/Traefik/Cloudflare) on port 8080:
  sudo $0 --no-tls --http-port 8080
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode)       MODE="$2"; shift 2 ;;
    --domain)     DOMAIN="$2"; shift 2 ;;
    --no-tls)     NO_TLS="true"; shift ;;
    --http-port)  HTTP_PORT="$2"; shift 2 ;;
    -h|--help)    usage; exit 0 ;;
    *)            echo "Unknown option: $1"; usage; exit 1 ;;
  esac
done

if [[ ! -f "${WEBSITE_SRC}/index.html" ]]; then
  echo "Error: Could not find ${WEBSITE_SRC}/index.html."
  echo "Run this script from inside the BlockGame repository."
  exit 1
fi

if [[ "${NO_TLS}" == "true" ]]; then
  SITE_ADDRESS=":${HTTP_PORT}"
else
  SITE_ADDRESS="${DOMAIN}, www.${DOMAIN}"
fi

# Auto-detect best install mode
if [[ "${MODE}" == "auto" ]]; then
  if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
    MODE="docker"
  else
    MODE="native"
  fi
fi

echo "============================================================"
echo "  Roots & Ridges Website Installer"
echo "  Mode:    ${MODE}"
echo "  Address: ${SITE_ADDRESS}"
echo "============================================================"

if [[ "${MODE}" == "docker" ]]; then
  if ! command -v docker >/dev/null 2>&1; then
    echo "Docker not found. Installing Docker via get.docker.com..."
    curl -fsSL https://get.docker.com | sh
  fi

  cd "${REPO_ROOT}"
  if [[ "${NO_TLS}" == "true" ]]; then
    export SITE_ADDRESS=":80"
    export WEBSITE_HTTP_PORT="${HTTP_PORT}"
    export WEBSITE_HTTPS_PORT="${HTTPS_PORT}"
  else
    export SITE_ADDRESS="${DOMAIN}, www.${DOMAIN}"
    export WEBSITE_HTTP_PORT="${HTTP_PORT}"
    export WEBSITE_HTTPS_PORT="${HTTPS_PORT}"
  fi

  echo "Building and starting Docker container (rootsandridges_website)..."
  docker compose -f docker-compose.website.yml up -d --build

  echo ""
  echo "Done! Container status:"
  docker compose -f docker-compose.website.yml ps

elif [[ "${MODE}" == "native" ]]; then
  if [[ "${EUID}" -ne 0 ]]; then
    echo "Error: Native install requires root privileges. Please run with sudo:"
    echo "  sudo $0 $*"
    exit 1
  fi

  # Install Caddy on Debian/Ubuntu if not already present
  if ! command -v caddy >/dev/null 2>&1; then
    echo "Installing Caddy web server..."
    apt-get update -qq
    apt-get install -y -qq debian-keyring debian-archive-keyring apt-transport-https curl gnupg
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | tee /etc/apt/sources.list.d/caddy-stable.list >/dev/null
    apt-get update -qq
    apt-get install -y -qq caddy
  fi

  echo "Syncing website files to ${INSTALL_DIR}..."
  mkdir -p "${INSTALL_DIR}"
  cp -a "${WEBSITE_SRC}/." "${INSTALL_DIR}/"
  chown -R caddy:caddy "${INSTALL_DIR}"

  echo "Writing /etc/caddy/Caddyfile..."
  cat > /etc/caddy/Caddyfile <<EOF
${SITE_ADDRESS} {
    root * ${INSTALL_DIR}
    encode zstd gzip

    @static path *.png *.jpg *.jpeg *.svg *.mp3 *.mp4 *.ico *.woff2
    header @static Cache-Control "public, max-age=2592000, immutable"

    @web path *.html *.css *.js
    header @web Cache-Control "public, max-age=300, must-revalidate"

    header {
        X-Content-Type-Options "nosniff"
        X-Frame-Options "SAMEORIGIN"
        Referrer-Policy "strict-origin-when-cross-origin"
        -Server
    }

    file_server
}
EOF

  caddy validate --config /etc/caddy/Caddyfile
  systemctl enable --now caddy
  systemctl reload caddy

  echo ""
  echo "Done! Caddy service status:"
  systemctl --no-pager -l status caddy | head -n 12
else
  echo "Error: Unknown mode '${MODE}'. Use 'docker', 'native', or 'auto'."
  exit 1
fi

echo ""
echo "============================================================"
if [[ "${NO_TLS}" == "true" ]]; then
  echo "  Roots & Ridges is live on HTTP port ${HTTP_PORT}!"
  echo "  Point your reverse proxy to http://127.0.0.1:${HTTP_PORT}"
else
  echo "  Roots & Ridges is configured for https://${DOMAIN}"
  echo "  Make sure your DNS A records point to this server's IP:"
  echo "    A   @     -> <YOUR_SERVER_IP>"
  echo "    A   www   -> <YOUR_SERVER_IP>"
  echo "  Ensure ports 80 and 443 are open in your firewall."
fi
echo "  To update the site later: git pull && sudo $0"
echo "============================================================"
