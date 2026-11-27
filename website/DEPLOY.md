# Deploying Roots & Ridges (`rootsandridges.gg`)

The website in `website/` is a self-contained static site (HTML, CSS, JS, images, stereo soundtrack, and trailer). You can install it on your server in one command with **automatic Let's Encrypt HTTPS** via Caddy.

---

## Option 1: One-Command Installer (Recommended)

On your Linux server, clone the repository and run the installer:

```bash
git clone https://github.com/Mjwitz/BlockGame.git
cd BlockGame
sudo ./scripts/install_website.sh
```

- Automatically uses **Docker Compose** if Docker is installed, or installs **native Caddy** as a `systemd` service if not.
- Automatically obtains and renews free HTTPS certificates for `rootsandridges.gg` and `www.rootsandridges.gg`.

### Already running Nginx, Traefik, or Cloudflare Tunnel?
Run the site on a local port (e.g. `8080`) without TLS and proxy to `http://127.0.0.1:8080`:

```bash
sudo ./scripts/install_website.sh --no-tls --http-port 8080
```

---

## Option 2: Docker Compose Directly

Matches the existing `docker-compose.directory.yml` workflow:

```bash
docker compose -f docker-compose.website.yml up -d --build
```

---

## DNS & Firewall Checklist

1. Point your domain's DNS records to your server's public IP:
   - `A` record for `@` (`rootsandridges.gg`) → `<YOUR_SERVER_IP>`
   - `A` record for `www` (`www.rootsandridges.gg`) → `<YOUR_SERVER_IP>`
2. Open ports **80** and **443** (TCP & UDP) in your firewall (`ufw allow 80 && ufw allow 443`).

## Updating the Website Later

```bash
cd BlockGame
git pull
sudo ./scripts/install_website.sh
```
