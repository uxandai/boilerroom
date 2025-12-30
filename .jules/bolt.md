## 2024-05-23 - Sequential IO in React Components
**Learning:** Avoid `await` inside loops in React components, especially when updating state. This causes "waterfall" network/IO requests and N+1 re-renders, significantly degrading performance.
**Action:** Use `Promise.all` to parallelize independent async operations and batch state updates to trigger a single re-render.
