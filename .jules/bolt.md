## 2024-05-23 - Waterfall Re-renders in List Components
**Learning:** React components (like `LibraryPanel`) that iterate over a list and update state inside the loop (e.g., fetching artwork one by one) cause a "waterfall of re-renders".
**Action:** Always batch state updates. If fetching data for a list, use `Promise.all` with a concurrency limit (e.g., chunks of 20) and update the state once per chunk. This reduces re-renders from N to N/chunk_size.
