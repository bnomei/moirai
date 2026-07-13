# Design — Moirai milestone hardening

This follow-up closes defects found by two independent read-only milestone reviews after spec 002.
It does not add consumer facades or tune runtime performance.

Resource scopes use an internal restoration guard whose drop path restores the taken resource and
clears the scoped sentinel during unwinding. Normal return preserves the existing changed-tick and
exact-drop semantics. Exact-ID planning rejects duplicates after owner/stale validation and before
any iterator or mutable borrow is produced.

App execution uses RAII cleanup independent of the optional `std` catch layer. Panic cleanup clears
run guards, commands, World and RunContext fixed-step state, and completes the documented failed
frame boundary. Std continues to rethrow after recording the first fault; default-feature builds
must also leave inspectable state coherent when a host catches an unwind. The schedule runner checks
World mutation poison immediately after each system body even when the body converts the originating
error into success.

The final task reconciles architecture and completed-task wording with the landed public API and
removes the feature-sensitive test warning. Every slice receives a distinct Sol/high validation and
commit.
