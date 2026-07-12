# Coverage policy

Source-line coverage runs on current stable only. It does not define the Rust 1.75 library MSRV
contract.

Phase 1 establishes the crate boundary without executable ECS behavior. The 100% source-line gate
begins once owning phases land real code paths. Until then, CI proves format, lint, feature matrix,
warnings-denied docs, benchmark-harness compilation, and the MSRV library check.