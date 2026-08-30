# PADAGONIA Integration Roadmap

Use `/home/sal/padagonia/docs/enterprise-integration-directives.md`.

## Modules

- `build_event_adapter`: map project, toolchain, artifact, size, and result.
- `maintenance_writer`: record cleanup candidates, operator decisions, and
  recoverability classification.
- `bloat_reader`: query temporal growth and recurring artifact sources.
- `evidence_export`: emit deterministic maintenance reports with source paths
  and scan timestamps.

## Acceptance gates

Cleanup remains dry-run by default, deletion decisions are auditable, and local
maintenance works if Padagonia is down.
