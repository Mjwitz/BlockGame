# Getting Started

## Prebuilt Releases (Recommended)

1. Go to the [Releases](https://github.com/Mjwitz/BlockGame/releases) page.
2. Download the archive for your operating system:
   - **Linux (`x86_64`)**: `BlockGame-linux-x86_64.tar.gz`
   - **Windows (`x86_64`)**: `BlockGame-windows-x86_64.zip`
3. Extract the archive:
   - **Linux**:
     ```bash
     tar -xzf BlockGame-linux-x86_64.tar.gz
     cd BlockGame-linux-x86_64
     ./blockgame_client
     ```
   - **Windows**: Extract the `.zip` file and double-click `blockgame_client.exe`.

---

## Building from Source

### 1. Install Rust
Install the stable Rust toolchain via [rustup.rs](https://rustup.rs/).

### 2. Install System Dependencies

#### Linux (Debian / Ubuntu)
```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libasound2-dev \
  libx11-dev libxi-dev libxcursor-dev libxrandr-dev libwayland-dev libxkbcommon-dev
```
*(Ensure Vulkan or OpenGL GPU drivers are installed for `wgpu`.)*

#### Windows
- Install the **Desktop development with C++** workload (MSVC build tools) via Visual Studio Installer, or use `rustup`'s default MSVC setup.
- DirectX 12 or Vulkan GPU drivers.

### 3. Clone and Build
```bash
git clone https://github.com/Mjwitz/BlockGame.git
cd BlockGame
cargo build --release -p blockgame_client -p blockgame_server
```

The compiled executables will be placed in `target/release/`:
- `blockgame_client` (or `blockgame_client.exe` on Windows)
- `blockgame_server` (or `blockgame_server.exe` on Windows)

### 4. Run Directly with Cargo
```bash
cargo run --release -p blockgame_client
```
