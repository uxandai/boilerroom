# GitHub Actions Workflows

## Release Workflow

**File:** `release.yml`

### How It Works

This workflow automatically builds and publishes a new release when you push a version tag.

### Triggering a Release

1. **Commit your changes** to the main branch
2. **Create and push a version tag**:
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```

3. **GitHub Actions will automatically**:
   - Build the .NET 8 application
   - Create a GitHub Release with the tag name
   - Upload `SolusManifestApp.exe` as a release asset

### Version Tag Format

Use semantic versioning: `v{major}.{minor}.{patch}`

Examples:
- `v1.0.0` - Initial release
- `v1.0.1` - Bug fix
- `v1.1.0` - New features
- `v2.0.0` - Breaking changes

### Auto-Update Integration

The app's `UpdateService` checks GitHub's `/releases/latest` endpoint, so users will automatically be notified of new versions when you:

1. Push a new tag (e.g., `v1.0.1`)
2. Workflow creates the release
3. Users with auto-update enabled get notified
4. One-click update downloads and installs the new version

### Testing the Workflow

After pushing a tag, you can:
1. Go to your GitHub repo → Actions tab
2. Watch the "Build and Release" workflow run
3. Once complete, check the Releases page for the new release

### Troubleshooting

If the workflow fails:
- Check the Actions tab for error logs
- Ensure your .NET project builds locally with `dotnet publish -c Release -r win-x64`
- Verify the tag format matches `v*.*.*` pattern
