# Requirements — Moirai milestone hardening

R001: WHEN a `resource_scope` callback unwinds under an unwind-capable host THE WORLD SHALL restore
  the scoped resource and clear its scope sentinel exactly once before the panic continues.
R002: WHEN an exact-ID query contains the same live entity more than once THE WORLD SHALL reject the
  query before iteration or mutation rather than yielding or mutating that entity repeatedly.
R003: WHEN a system catches a terminal change-tick exhaustion and returns success THE APP SHALL
  still retain a terminal fault before any later system or successful operation completion.
R004: WHEN a system or App observation callback unwinds THE APP SHALL clean World/run/fixed-step and
  frame-boundary state and retain the first panic fault on both std and default-feature unwind hosts.
R005: WHEN the hardening repairs run THEY SHALL preserve Rust 1.75, `no_std + alloc`, safe code,
  dependency purity, and the explicitly deferred performance-audit boundary.
R006: WHEN architecture, task, or API documentation describes integration contracts IT SHALL agree
  with the owner-bearing EntityId, actual builder/system methods, and executable public paths.
R007: WHEN no-default tests compile THE CRATE SHALL not emit the known feature-sensitive internal
  unused-import warning.
R008: WHEN a hardening task requests completion A fresh Sol/high validator SHALL independently
  inspect its report, diff, regression proofs, and focused/full validation before staging.
