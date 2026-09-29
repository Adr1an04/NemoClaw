# SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0
"""Packaging transports Fabric discovery without redefining adapter metadata."""

import json
import os
import tempfile
import unittest
from pathlib import Path

from catalog import snapshot
from nemo_fabric import DiscoveryConfig, Fabric


class CatalogContract(unittest.TestCase):
    def snapshot_with_runtime_files(self, declared):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory, "runtime-files.json")
            path.write_text(json.dumps(declared))
            return snapshot("a" * 40, "b" * 64, runtime_files=path)

    def test_runtime_files_are_recorded_beside_unedited_descriptors(self):
        records = [record.to_mapping() for record in Fabric().discover()]
        self.assertTrue(records, "requires a discoverable Fabric adapter")
        adapter_id = records[0]["descriptor"]["adapter_id"]
        catalog = self.snapshot_with_runtime_files({adapter_id: ["/opt/runtime"]})
        self.assertEqual(catalog["runtime_files"], {adapter_id: ["/opt/runtime"]})
        self.assertEqual(catalog["adapters"], records)

    def test_image_without_runtime_files_adds_no_field(self):
        with tempfile.TemporaryDirectory() as directory:
            catalog = snapshot("a" * 40, "b" * 64, runtime_files=Path(directory, "absent.json"))
        self.assertNotIn("runtime_files", catalog)

    def test_runtime_files_for_an_uncataloged_adapter_fail_the_snapshot(self):
        with self.assertRaisesRegex(ValueError, "org.fixture.absent"):
            self.snapshot_with_runtime_files({"org.fixture.absent": ["/opt/runtime"]})

    def test_relative_runtime_files_fail_the_snapshot(self):
        adapter_id = Fabric().discover()[0].to_mapping()["descriptor"]["adapter_id"]
        with self.assertRaisesRegex(ValueError, "absolute paths"):
            self.snapshot_with_runtime_files({adapter_id: ["opt/runtime"]})

    def test_parent_traversal_runtime_files_fail_the_snapshot(self):
        adapter_id = Fabric().discover()[0].to_mapping()["descriptor"]["adapter_id"]
        with self.assertRaisesRegex(ValueError, "absolute paths"):
            self.snapshot_with_runtime_files({adapter_id: ["/opt/runtime/../../etc"]})

    def test_runtime_files_must_be_a_list_of_paths(self):
        adapter_id = Fabric().discover()[0].to_mapping()["descriptor"]["adapter_id"]
        with self.assertRaisesRegex(ValueError, "absolute paths"):
            self.snapshot_with_runtime_files({adapter_id: {"/opt/runtime": True}})

    def test_snapshot_preserves_owner_records(self):
        records = Fabric().discover()
        catalog = snapshot("a" * 40, "b" * 64)
        self.assertEqual(catalog["adapters"], [record.to_mapping() for record in records])

    def test_snapshot_preserves_fabric_workflow_targets(self):
        records = Fabric().discover_targets()
        catalog = snapshot("a" * 40, "b" * 64)
        self.assertEqual(catalog["targets"], [record.to_mapping() for record in records])

    @unittest.skipUnless(
        os.environ.get("NEMOCLAW_TEST_FABRIC_DESCRIPTOR"), "requires Fabric fixture"
    )
    def test_new_fabric_descriptor_is_discovered_through_packaging(self):
        discovery = DiscoveryConfig(local_paths=[os.environ["NEMOCLAW_TEST_FABRIC_DESCRIPTOR"]])
        records = Fabric().discover(discovery=discovery)
        catalog = snapshot("a" * 40, "b" * 64, discovery=discovery)
        self.assertEqual(catalog["adapters"], [record.to_mapping() for record in records])
        adapter_id = json.loads(Path(os.environ["NEMOCLAW_TEST_FABRIC_DESCRIPTOR"]).read_text())[
            "adapter_id"
        ]
        self.assertTrue(
            any(record["descriptor"]["adapter_id"] == adapter_id for record in catalog["adapters"])
        )
