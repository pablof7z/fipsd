# Measured performance and resource envelope

The checked-in benchmark at `fixtures/m8/benchmark-macos-aarch64.json` was
measured on macOS arm64 in a debug build on 2026-07-22. Timings apply only to
that recorded host/build and are not release-binary claims.

| Case | Representation | p50 | p95 | Output bytes |
| --- | --- | ---: | ---: | ---: |
| Artifact analysis | 12-node exact artifact | 1.386 ms | 1.454 ms | 9,892 |
| Bounded event query | 100 sampled events | 0.109 ms | 0.109 ms | 100 sampled records |
| Ten-family atlas | Deterministic models | 53.482 ms | 55.412 ms | 50,937 |
| Million-node scale | Cohort | 1.233 ms | 1.295 ms | 21,705 |
| Billion-node scale | Cohort | 1.458 ms | 1.520 ms | 25,956 |

The million- and billion-node rows measure cohort execution: represented
population mass, not allocated node objects. Their metrics carry deterministic
bounds and calibration caveats. The benchmark records input/output bytes as a
portable memory lower bound; peak RSS is explicitly unobserved.

Run a release-build measurement on the target host before making a local
performance claim:

```bash
cargo build --locked --release -p fips-cli --bin fips-wind-tunnel
target/release/fips-wind-tunnel release benchmark \
  --iterations 100 --output benchmark.json
```

All bounded engines check allocation, event, evaluation, or wall-time budgets.
Resource failures return typed errors and do not synthesize partial success.
Previously written immutable evidence remains available for diagnosis.
