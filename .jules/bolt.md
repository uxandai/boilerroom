## 2024-05-23 - [Parallel Artwork Fetching]
**Learning:** React state updates inside loops (N+1 re-renders) are a major bottleneck. Batching asynchronous operations and their state updates significantly improves performance.
**Action:** When handling lists of async tasks, always batch them (Promise.all) and update state once (or in chunks) to minimize React render cycles.
