## 2024-05-22 - Library List Virtualization & Memoization
**Learning:** Batch updates to `artworkMap` were triggering full list re-renders because game items were rendered inline. This is a common bottleneck in React lists where item state (image) is loaded asynchronously.
**Action:** Extract list items to memoized components (`React.memo`) and pass only the specific data needed (e.g. `artworkUrl` string instead of the whole map) to isolate re-renders.
