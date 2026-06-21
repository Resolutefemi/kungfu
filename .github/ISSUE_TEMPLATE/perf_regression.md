---
name: Performance regression
about: Report a throughput or latency regression
title: "[perf] "
labels: performance
assignees: ''
---

## Summary

Brief description of the performance regression.

## Before / after numbers

| Metric | Before | After | Change |
|---|---|---|---|
| Throughput (req/s) | | | |
| p50 latency | | | |
| p99 latency | | | |
| Memory usage | | | |

## Benchmark command

```bash
# How you ran the benchmark
oha -z 10s -c 256 http://localhost:3000/hello
```

## Environment

- Hardware: [e.g. 16-core AMD EPYC, 64GB RAM]
- OS: [e.g. Ubuntu 22.04, kernel 5.15]
- Kungfu version + features: [e.g. 0.1.0 with io_uring + simd]
- Load generator: [e.g. oha 1.4.0, wrk 4.2.0]

## Suspected cause

If you have a hypothesis about what caused the regression, describe it.

## Profile output

If you have flamegraph or perf output, paste it here.
