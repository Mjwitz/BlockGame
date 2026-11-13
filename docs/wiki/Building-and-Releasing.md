# Building and Releasing

BlockGame uses GitHub Actions ([`.github/workflows/release.yml`](https://github.com/Mjwitz/BlockGame/blob/main/.github/workflows/release.yml)) to automatically compile and package release binaries for both **Linux (`x86_64`)** and **Windows (`x86_64`)**.

## Automated GitHub Releases

### Option A: Publish a Tagged Release
Push a version tag starting with `v` to trigger the workflow and publish a GitHub Release with downloadable archives:

```bash
git tag v0.1.0
git push origin v0.1.0
```

Once the workflow completes, the release will contain:
- `BlockGame-linux-x86_64.tar.gz` (`blockgame_client`, `blockgame_server`, `README.txt`)
- `BlockGame-windows-x86_64.zip` (`blockgame_client.exe`, `blockgame_server.exe`, `README.txt`)

### Option B: Manual Workflow Dispatch (Artifacts Only)
1. Open the **Actions** tab on GitHub.
2. Select **Build and Release Executables**.
3. Click **Run workflow**.
4. Download `BlockGame-linux-x86_64` and `BlockGame-windows-x86_64` from the workflow run's **Artifacts** section.

## Release Profile Optimization
The workspace [`Cargo.toml`](https://github.com/Mjwitz/BlockGame/blob/main/Cargo.toml) configures `[profile.release]` with:
- `opt-level = 3`
- `lto = "thin"`
- `codegen-units = 1`
- `strip = true`

This keeps distributed client and server executables compact and fast.
