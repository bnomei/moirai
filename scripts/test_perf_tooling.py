from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import pathlib
import tempfile
import unittest
from unittest import mock


SCRIPTS_DIR = pathlib.Path(__file__).parent


def load_script(name: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS_DIR / f"{name}.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class PerfExperimentTests(unittest.TestCase):
    def test_alternates_order_and_continues_after_failure(self) -> None:
        module = load_script("perf_experiment")
        results = [
            mock.Mock(returncode=9),
            mock.Mock(returncode=0),
            mock.Mock(returncode=0),
            mock.Mock(returncode=0),
        ]
        with mock.patch.object(module.subprocess, "run", side_effect=results) as run:
            status = module.main(
                [
                    "--group",
                    "query",
                    "--cycles",
                    "2",
                    "--cohort",
                    "cohort-a",
                    "--baseline-command",
                    "bench --name 'baseline one'",
                    "--candidate-command",
                    "bench --name candidate",
                ]
            )

        self.assertEqual(status, 1)
        labels = []
        for call in run.call_args_list:
            invocation = call.args[0]
            labels.append(invocation[invocation.index("--label") + 1])
            self.assertEqual(
                invocation[invocation.index("--cohort") + 1], "cohort-a"
            )
            self.assertEqual(call.kwargs, {"check": False})
        self.assertEqual(
            labels,
            [
                "query-baseline-1",
                "query-candidate-1",
                "query-candidate-2",
                "query-baseline-2",
            ],
        )
        self.assertIn("baseline one", run.call_args_list[0].args[0])

    def test_cycles_must_be_positive(self) -> None:
        module = load_script("perf_experiment")
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            module.parse_arguments(
                [
                    "--group",
                    "query",
                    "--cycles",
                    "0",
                    "--baseline-command",
                    "baseline",
                    "--candidate-command",
                    "candidate",
                ]
            )


class PerfCaptureTests(unittest.TestCase):
    def test_launch_error_still_writes_logs_and_completed_metadata(self) -> None:
        module = load_script("perf_capture")
        with tempfile.TemporaryDirectory() as temporary:
            output_dir = pathlib.Path(temporary) / "captures"
            argv = [
                "perf_capture.py",
                "--label",
                "launch-error",
                "--cohort",
                "cohort-a",
                "--output-dir",
                str(output_dir),
                "--",
                "missing-benchmark",
            ]
            with (
                mock.patch.object(module.sys, "argv", argv),
                mock.patch.object(module, "run_metadata_command", return_value=None),
                mock.patch.object(module, "executable_metadata", return_value=None),
                mock.patch.object(module, "capture_working_tree", return_value={}),
                mock.patch.object(module.subprocess, "run", side_effect=OSError("not found")),
                contextlib.redirect_stderr(io.StringIO()),
            ):
                status = module.main()

            captures = list(output_dir.iterdir())
            self.assertEqual(len(captures), 1)
            capture = captures[0]
            metadata = json.loads((capture / "metadata.json").read_text(encoding="utf-8"))
            self.assertEqual(status, 127)
            self.assertEqual((capture / "stdout.log").read_bytes(), b"")
            self.assertEqual((capture / "stderr.log").read_text(encoding="utf-8"), "not found\n")
            self.assertEqual(metadata["status"], "launch-error")
            self.assertEqual(metadata["exit_code"], 127)
            self.assertEqual(metadata["error"], "not found")
            self.assertEqual(metadata["cohort"], "cohort-a")
            self.assertIn("completed_at_utc", metadata)


class PerfSummarizeTests(unittest.TestCase):
    def write_capture(
        self,
        root: pathlib.Path,
        directory: str,
        label: str,
        status: str,
        exit_code: int,
        median: str | None,
        start_load: float,
        end_load: float,
        cohort: str | None = None,
    ) -> None:
        capture = root / directory
        capture.mkdir()
        (capture / "metadata.json").write_text(
            json.dumps(
                {
                    "label": label,
                    "cohort": cohort,
                    "status": status,
                    "exit_code": exit_code,
                    "duration_seconds": 1.25,
                    "load_average_at_start": [start_load, 0.0, 0.0],
                    "load_average_at_end": [end_load, 0.0, 0.0],
                }
            ),
            encoding="utf-8",
        )
        if median is not None:
            (capture / "stdout.log").write_text(
                "case  1 ns │ 3 ns │ " + median + " │ 2 ns │ 100 │ 1\n",
                encoding="utf-8",
            )

    def test_reports_failures_statistics_pairs_and_load_without_gates(self) -> None:
        module = load_script("perf_summarize")
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            self.write_capture(root, "a", "query-baseline-1", "passed", 0, "10 ns", 1.0, 1.2)
            self.write_capture(root, "b", "query-candidate-1", "failed", 7, "8 ns", 8.0, 9.0)
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                status = module.main([str(root)])

        report = output.getvalue()
        self.assertEqual(status, 0)
        self.assertIn("[capture_inventory]", report)
        self.assertIn(
            "uncohorted\tquery-candidate-1\tquery\tcandidate\t1\tfailed\t7\t1.25\t8.000\t9.000",
            report,
        )
        self.assertIn("[capture_failures]", report)
        self.assertIn("uncohorted\tquery\tcase\tbaseline\tpassed\t1\t10.000\t0.000\t10.000\t10.000", report)
        self.assertIn("uncohorted\tquery\tcase\tcandidate\tfailed\t1\t8.000\t0.000\t8.000\t8.000", report)
        self.assertIn(
            "uncohorted\tquery\tcase\t1\t1\ta\tpassed\tb\tfailed\t10.000\t8.000\t-2.000\t-20.00",
            report,
        )
        self.assertNotIn("\tgate", report)

    def test_repeated_labels_remain_distinct_and_pair_within_cohort(self) -> None:
        module = load_script("perf_summarize")
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            self.write_capture(
                root, "a-baseline", "query-baseline-1", "passed", 0,
                "10 ns", 1.0, 1.0, "a",
            )
            self.write_capture(
                root, "b-candidate", "query-candidate-1", "passed", 0,
                "25 ns", 1.0, 1.0, "b",
            )
            self.write_capture(
                root, "a-candidate", "query-candidate-1", "passed", 0,
                "8 ns", 1.0, 1.0, "a",
            )
            self.write_capture(
                root, "b-baseline", "query-baseline-1", "passed", 0,
                "20 ns", 1.0, 1.0, "b",
            )
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                status = module.main([str(root)])

        report = output.getvalue()
        self.assertEqual(status, 0)
        self.assertIn("a\tquery\tcase\tbaseline\tpassed\t1\t10.000", report)
        self.assertIn("a\tquery\tcase\tcandidate\tpassed\t1\t8.000", report)
        self.assertIn("b\tquery\tcase\tbaseline\tpassed\t1\t20.000", report)
        self.assertIn("b\tquery\tcase\tcandidate\tpassed\t1\t25.000", report)
        self.assertNotIn("\t2\t15.000", report)
        self.assertNotIn("\t2\t16.500", report)
        self.assertIn(
            "a\tquery\tcase\t1\t1\ta-baseline\tpassed\ta-candidate\tpassed\t10.000\t8.000\t-2.000\t-20.00",
            report,
        )
        self.assertIn(
            "b\tquery\tcase\t1\t1\tb-baseline\tpassed\tb-candidate\tpassed\t20.000\t25.000\t+5.000\t+25.00",
            report,
        )

    def test_zero_baseline_reports_absolute_delta_and_undefined_percentage(self) -> None:
        module = load_script("perf_summarize")
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            self.write_capture(
                root, "baseline", "query-baseline-1", "passed", 0,
                "0 ns", 1.0, 1.0, "zero",
            )
            self.write_capture(
                root, "candidate", "query-candidate-1", "passed", 0,
                "5 ns", 1.0, 1.0, "zero",
            )
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                status = module.main([str(root)])

        self.assertEqual(status, 0)
        self.assertIn(
            "zero\tquery\tcase\t1\t1\tbaseline\tpassed\tcandidate\tpassed\t0.000\t5.000\t+5.000\tundefined",
            output.getvalue(),
        )


if __name__ == "__main__":
    unittest.main()
