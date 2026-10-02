# SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0
"""Bound Docker fixture commands, preserving argv and binary stdin without a shell."""

import json
import subprocess
import sys

request = json.load(sys.stdin)
result = subprocess.run(
    ["docker", "--host", "unix:///var/run/docker.sock", *request["args"]],
    input=bytes(request.get("stdin", [])),
    capture_output=True,
    timeout=90,
    check=False,
)
json.dump(
    {"code": result.returncode, "stdout": list(result.stdout), "stderr": list(result.stderr)},
    sys.stdout,
)
