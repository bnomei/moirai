# Replace the unreproducible median threshold with retained descriptive captures

Priority: high
Confidence: high
Status: implemented as a descriptive protocol; no timing admission gate

## Finding

The former fixed same-machine median rule had no demonstrated noise floor. Earlier saved results also
mixed Divan defaults with explicit sample descriptions, and an unchanged Q16 source produced a
large unexplained cross-session shift. A fixed percentage could therefore classify host or artifact
drift as a source-code regression or improvement.

## Protocol

Run seven positive paired cycles. `perf_experiment.py` alternates AB then BA ordering to expose
directional thermal drift. `perf_capture.py` records the exact argv, working-tree identity, executable
metadata when available, toolchain, target, feature selection, power note, timestamps, load average,
exit status, and raw stdout/stderr.

All endpoints are evidence. Busy-host samples, outliers, parse-empty output, and failed commands are
retained. There is no pre-launch load rejection, no post-hoc exclusion, and no automatic threshold.
`perf_summarize.py` reports inventory, failures, descriptive median/MAD/min/max statistics separated
by endpoint status, and matched deltas carrying both baseline and candidate status. It emits no gate.

## Exact paired commands

The feature-gated `query1_paired_control` runs the retained internal ad-hoc path and reusable prepared
path behind one Divan case identity, so the summary can match corresponding rows:

```sh
uv run python scripts/perf_experiment.py \
  --group prepared-query1 \
  --cycles 7 \
  --power-note "record current power and thermal context" \
  --output-dir target/perf-results/prepared-query1 \
  --baseline-command "env MOIRAI_QUERY_CONTROL=adhoc cargo bench --features bench-internals --bench prepared_queries -- query1_paired_control --sample-count 100 --sample-size 100" \
  --candidate-command "env MOIRAI_QUERY_CONTROL=prepared cargo bench --features bench-internals --bench prepared_queries -- query1_paired_control --sample-count 100 --sample-size 100"
uv run python scripts/perf_summarize.py target/perf-results/prepared-query1
```

For a revision-to-revision experiment, make both commands invoke already-built benchmark executables
from the two revisions with identical Divan arguments and case names. Do not alternate source edits or
builds inside timed commands.

## Interpretation

The distributions support engineering judgment and identify unstable measurements; they do not by
themselves accept or reject an optimization. Allocation counts, removed work, correctness contracts,
profiles, and representative end-to-end behavior remain separate evidence. Compile-only CI remains a
portable benchmark build check and provides no performance verdict.
