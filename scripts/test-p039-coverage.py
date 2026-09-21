#!/usr/bin/env python3
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("coverage", Path(__file__).with_name("check-p039-coverage.py"))
coverage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(coverage)


class ExecutionCoverage(unittest.TestCase):
    def setUp(self):
        self.mapping = {f"CF-{index:02}": [f"case_{index}"] for index in range(1, 30)}
        self.log = "\n".join(f"test case_{index} ... ok" for index in range(1, 30))

    def test_all_cases_require_executed_success(self):
        self.assertEqual(coverage.check(self.mapping, self.log), (29, 29))

    def test_zero_tests_filtered_ignored_failed_and_duplicate_events_fail(self):
        for log in ["running 0 tests", self.log.replace("case_2 ... ok", "case_2 ... ignored"),
                    self.log.replace("case_2 ... ok", "case_2 ... FAILED"),
                    self.log.replace("test case_2 ... ok", "case_2: test"),
                    self.log + "\ntest case_2 ... ok"]:
            with self.subTest(log=log):
                with self.assertRaises(ValueError):
                    coverage.check(self.mapping, log)

    def test_missing_case_empty_list_and_duplicate_mapping_fail(self):
        for mapping in [{}, {**self.mapping, "CF-30": ["extra"]},
                        {**self.mapping, "CF-01": []}, {**self.mapping, "CF-01": ["case_1", "case_1"]}]:
            with self.assertRaises(ValueError):
                coverage.check(mapping, self.log)


if __name__ == "__main__":
    unittest.main()
