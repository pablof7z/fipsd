# v0.1 support matrix

| Surface | v0.1 status | Evidence or boundary |
| --- | --- | --- |
| macOS arm64/x86_64 CLI | Supported | Workspace tests, release audit, clean-install script |
| Linux arm64/x86_64 CLI | Supported | CI matrix and release packaging workflow |
| Windows CLI | Unsupported | No build or determinism matrix |
| Static analysis browser | Supported | No-server export, local file parser, immutable analysis document |
| Individual semantic engine | Supported to declared campaign/event budgets | M1/M2 fixtures and assertions |
| Cohort/hybrid engine | Supported with explicit uncertainty | M4 calibration and sampled exact evidence |
| One-billion-node analysis | Cohort/hybrid only | Bounded M4 demo; no individual claim |
| Pinned FIPS daemon | Supported at commit `80c956a6...3267` | M5 live smoke and scheduled oracle |
| UDP/TCP abstract transports | Supported | Deterministic link/session model |
| Ethernet/BLE/Tor/Nym real calibration | Unsupported | Abstract labels only unless a calibration is attached |
| Dynamic protocol plugins | Unsupported | v0.1 variants are compiled trusted code |
| Campaign schema | `v1alpha1` | Unknown versions and fields rejected |
| Run/reproduction artifacts | `v1alpha1` | Readers reject unknown versions; sources remain immutable |

Exact semantic results require byte equality. Statistical, calibrated, cohort,
and hybrid results compare only through their declared range and uncertainty.
Unknown or unsupported observations are never rendered as zero or agreement.
