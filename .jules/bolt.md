## 2025-05-21 - [React List Virtualization & Memoization]
**Learning:** Even with batched state updates for asynchronous data (like artwork), inline `sort()` and `filter()` operations in the render loop force O(N log N) recalculations on every batch update, negating performance benefits.
**Action:** Always move list transformation logic (sort/filter) into `useMemo` and extract list items into `React.memo` components when the list is long or when auxiliary state (like images) updates frequently independently of the list data.
