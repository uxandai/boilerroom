## 2024-05-23 - LibraryPanel List Virtualization & Memoization
**Learning:** React re-renders the entire list when a single item updates (e.g. artwork loading) if list items are defined inline.
**Action:** Always extract list items to `React.memo` components when the list is large or items update frequently/asynchronously.
**Verification Limitation:** Tauri apps are hard to verify with Playwright because they depend on the Tauri Rust backend and specific window objects (`window.__TAURI__`). Playwright tests are good for initial render but fail on deeper interaction without heavy mocking.
