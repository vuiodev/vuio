# VuIO Memory Usage Analysis

Detailed breakdown of memory consumption for the **VuIO Media Server** running in release mode.

## Test Environment & Process Overview

* **Command:** `./target/release/vuio`
* **Binary Size:** 13.7 MB (Mach-O ARM64 executable)
* **Active Library:** 351 media files and 14 directories indexed in SQLite (`media.db`)
* **Platform:** macOS 26.6 (Apple Silicon ARM64)

---

## 1. High-Level Memory Metrics

| Metric | Measured Value | Description |
| :--- | :--- | :--- |
| **Resident Set Size (RSS)** | **~17.6 MB** (18,032 KB) | Total physical memory pages mapped to the process |
| **Physical Footprint (`phys_footprint`)** | **12.0 MB** | Real unshared dirty memory held by the process |
| **Clean / Shared Memory** | **~7.5 MB** | Read-only machine code pages paged from disk on demand |
| **Virtual Memory Size (VSZ)** | **~425 MB** | Reserved address space (guard pages, virtual thread stack limits) |

---

## 2. Memory Subsystem Breakdown

### A. Clean / Read-Only Memory (~7.5 MB)
Memory that does not consume exclusive RAM and is paged directly from disk:
* **Executable Code (`__TEXT`):** ~7.38 MB
  * Rust binary machine code (`vuio-core`, `axum`, `tokio`, `rusqlite`, `rustls`, etc.)
  * Linked system libraries (`libsystem_pthread`, `libdyld`, `CoreFoundation`, `libobjc`, etc.)
* **Memory-Mapped Files (`mapped file`):** ~112 KB
  * SQLite write-ahead log coordination file (`media.db-shm`)
  * System logging cache plists

---

### B. Heap Allocations (~9.3 MB Dirty / ~4.21 MB Active Live Objects)
The process maintains **14,189 active heap allocations** totaling **4.21 MB** of live Rust structures. The remaining ~5.1 MB in `MALLOC_SMALL` represents page magazine caching and freelist pools maintained by Apple's `libsystem_malloc` for low-latency allocation recycling.

#### Allocation Breakdown by Component

| Component | Est. Memory | Description |
| :--- | :--- | :--- |
| **Media Metadata & In-Memory Records** | **~1.2 – 1.5 MB** | 351 indexed media items, directory hierarchy, MIME-type maps, and search indices. |
| **Tokio Async Runtime & Task Queues** | **~1.0 MB** | Work-stealing queues across 8 worker threads, task channels, timers, and I/O polling buffers. *(Largest single heap block is 320 KB for task/IO buffers)*. |
| **HTTP Server & Routing (`axum` / `hyper` / `rustls`)** | **~0.6 MB** | Request router trees, TLS context, headers, live configuration caches, and `AppState` synchronization locks. |
| **File System Watchers (`notify-rs` / FSEvents)** | **~0.4 MB** | FSEvents event-stream state, debouncer buffers, and directory change queues for `/Users/alex/Downloads` and config directory. |
| **SQLite Connection & Statement Cache (`rusqlite`)** | **~0.4 MB** | Connection handles, prepared statement caches, and SQLite B-tree page cache buffers for `media.db` & `media.db-wal`. |
| **Network Discovery & Casting** | **~0.3 MB** | Multicast DNS (`mDNS`) socket buffers, SSDP/UPnP device discovery listeners, and persistent renderer cache. |
| **macOS / CoreFoundation Primitives** | **~0.2 MB** | OS-level strings (`CFString`), dispatch semaphores, XPC dictionaries, and thread synchronization mutexes. |

#### Allocation Size Distribution
* **Small nodes (16 B – 128 B):** ~11,000 nodes (~2.8 MB) — `Arc` pointers, strings, small structs, hash map buckets.
* **Medium nodes (256 B – 4 KB):** ~3,100 nodes (~1.0 MB) — task frames, cached records, statement handles.
* **Large nodes (> 4 KB):** ~50 nodes (~0.4 MB) — buffers, hash table capacities, SQLite page frames.

---

### C. Thread Stacks (~736 KB Resident across 23 Threads)
The process runs **23 active threads**, each consuming ~32 KB resident stack memory:

1. **1 Main Thread:** Root process loop handling shutdown signals (`SIGINT` / `SIGTERM`).
2. **8 Tokio Worker Threads (`tokio-rt-worker`):** Multi-threaded async runtime execution pool.
3. **4 File Watcher Threads (`notify-rs`):**
   * 2 FSEvents event loop threads (kernel event listeners)
   * 2 Debouncer worker threads (file change batching)
4. **8 Tokio Blocking Worker Threads:** Background threadpool spawned for disk I/O and SQLite transactions during initial startup scanning.
5. **1 `mDNS_daemon` Thread:** Multicast DNS daemon for local network device discovery.
6. **1 System Workqueue Thread:** OS dispatch thread.

---

### D. Static Data & Linker Relocations (~1.0 MB Dirty)
* **`__DATA`, `__DATA_DIRTY`, `__DATA_CONST`:** Rust standard library global state, atomic counters, `rustls` cryptography lookup tables, and dynamic linker binding tables.

---

## 3. Empirical Benchmark & Scaling (10 Million Objects in DB)

A 10,000,000-object test library was generated using the native Rust benchmark tool (`benchmark_media.rs`) with full metadata, directory hierarchies, and B-Tree indexes. The release build of `vuio` was then launched over this 10M library and profiled live with macOS kernel diagnostics (`footprint`, `heap`, `vmmap`, `ps`).

### Benchmark Run Results

* **Total Records Inserted:** 10,000,000 media files
* **Insert Throughput (Rust + SQLite):** ~337,000 – 444,000 rows/sec (29.7 seconds total insertion time)
* **Index Creation Time:** 83.4 seconds (covering 10 B-tree & natural collation indexes)
* **Final Database File Size on Disk (`media.db`):** **7.82 GB** (8,007.7 MB)

### Measured Live Process Metrics at 10M Scale

| Metric | 351 Files (Baseline) | 10,000,000 Objects (Measured) | Description |
| :--- | :--- | :--- | :--- |
| **Resident Set Size (RSS)** | **~17.6 MB** | **172.1 MB** (172,160 KB) | Physical pages mapped into process RAM |
| **Physical Footprint (`phys_footprint`)** | **12.0 MB** | **164.0 MB** | Real unshared dirty memory |
| **Clean / Shared Memory (`__TEXT`)** | **~7.5 MB** | **~3.9 MB** | Read-only machine code pages paged on demand |
| **Virtual Memory Size (VSZ)** | **~425 MB** | **435 MB** | Reserved address space |
| **SQLite Page Cache (`MALLOC_SMALL`)** | **~2.6 MB** | **161.0 MB** (31,733 pages) | Governed by `database.cache_mb = 128` |

### Detailed Footprint Breakdown at 10,000,000 Objects

```
  Dirty Memory Category       Measured Size    Notes
  -------------------------   -------------    --------------------------------------------------
  MALLOC_SMALL                161.0 MB         SQLite B-tree page cache (31,733 x 5KB page nodes)
  MALLOC metadata             912 KB           libsystem_malloc zone headers & freelists
  __DATA_DIRTY                396 KB           Global mutable variables & atomic counters
  page table                  385 KB           Kernel virtual memory translation table
  __DATA_CONST                352 KB           Read-only relocations & constants
  stack                       336 KB           Active thread stack frames across worker threads
  __DATA                      202 KB           Static segments
  MALLOC_TINY                 144 KB           Small allocations (< 128 B)
  untagged (VM_ALLOCATE)      112 KB           Async runtime work buffers
  mapped file                 112 KB           SQLite coordination files (.db-shm, .db-wal)
  -------------------------   -------------    --------------------------------------------------
  TOTAL DIRTY FOOTPRINT       164.0 MB         Total exclusive physical RAM consumed
```

### Why RAM Remains Constrained at 10M Scale

1. **Zero In-Memory Media Collections:**
   * Media records and folder hierarchies remain entirely within SQLite tables (`media_files`, `directories`, `directory_mime_counts`).
   * Read sessions borrow directly out of SQLite statement result buffers without materializing full `Vec<MediaFile>` collections in heap memory.
   * Browsing and search queries use indexed B-tree scans with SQL `LIMIT` / `OFFSET` pagination.

2. **Hard-Capped Runtime Caches:**
   * **SOAP/DLNA Browse Response Cache (`BrowseResponseCache`):** Hard limit of **16 MB** (`BROWSE_CACHE_MAX_BYTES = 16 * 1024 * 1024`, max 256 entries).
   * **Bookmark Registry:** Hard limit of **10,000 entries** (`BOOKMARK_MAX_ENTRIES`).
   * **Active Casts & Renderers:** Hard limit of **128 entries** (`ACTIVE_CAST_MAX_ENTRIES`).

3. **Tunable SQLite Page Cache (`cache_mb`):**
   * Configured via `database.cache_mb` (default `128` MB per connection ceiling, applied via `PRAGMA cache_size = -{cache_kib}`).
   * Out of the **164 MB** footprint, **161 MB** is the SQLite page cache. Lowering `cache_mb` to `32` or `64` in `config.toml` reduces the active footprint to **~45 – 75 MB** even on an 8 GB database.

---

## 4. Profiling Commands for Reproduction

To inspect memory consumption live on macOS:

```bash
# Process RSS and VSZ
ps -o pid,rss,vsz,command -p <PID>

# Detailed OS memory footprint
footprint <PID>

# Virtual memory regions and resident pages
vmmap --summary <PID>
vmmap --wide <PID>

# Heap allocation details and object types
heap <PID>
heap --addresses all <PID>

# Thread sample and callstacks
sample <PID> 1 1 -file /tmp/vuio_sample.txt
```
