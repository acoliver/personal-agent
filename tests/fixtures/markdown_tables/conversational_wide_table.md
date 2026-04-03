Here is a broader comparison table.

| Service | Tier | Region | Monthly Cost | Notes |
| :--- | :--- | :--- | ---: | :--- |
| Compute | Starter | us-east-1 | 24.50 | Baseline burstable capacity suitable for dev workloads and smoke-test pipelines. |
| Compute | Production | eu-west-1 | 189.99 | Includes reserved CPU headroom, larger ephemeral disk, and stricter autoscaling floor for peak hours. |
| Storage | Standard | global | 72.00 | Mixed read/write object profile with lifecycle policy moving cold objects after 30 days. |
| Analytics | Batch | us-central1 | 310.40 | Nightly ETL plus backfill windows; query slots are capped to control noisy-neighbor effects. |

We can slice by region if that helps planning.
