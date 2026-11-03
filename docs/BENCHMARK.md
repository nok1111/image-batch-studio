# Benchmark (M3.5)

Command (release build, generates its own JPEG inputs in a temp folder):

```sh
cargo run --release -p ibs-core --example bench -- [count] [width] [height] [quality] [threads]
# defaults: 1000 images, 4000×3000 (12 MP), WebP q80, all logical CPUs
```

The run measures the engine only (decode → encode lossy WebP → write temp → decode-verify → atomic rename), not input generation.

## Results

| Machine | Images | Result |
|---|---|---|
| Devin Linux VM, 8 vCPU | 100 × 12 MP JPEG → WebP q80 | 24.9 s · 4.0 img/s · 48 MP/s · 197 → 71 MB |
| Windows 10 Pro, Ryzen 7 5700X (16 threads) | 1000 × 12 MP JPEG → WebP q80 | 109.0 s · 9.18 img/s · 110.1 MP/s · 1970 → 721 MB |

Notes:
- Verification decodes every output, so it costs roughly one extra decode per file.
- AVIF is encoded with rav1e at speed 6 without assembly optimizations (no NASM needed at build time); it is much slower than WebP.
