<!-- SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved. -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Reference Agent Contract on Linux ARM64

On October 1, 2026, the dummy image and all ten interim agent images passed the shared command-contract suite on native Linux ARM64 with Docker 29.2.1 and Python 3.13.15 inside the images.
This qualifies the [image command interface](../design/fabric-management.md#image-contract-and-reference-implementation), including standalone validation, retained files, generation checks, and declared health support.
It does not qualify live OpenShell deployment or native readiness in the real adapters.

## Images and Sources

The local tags use the prefix `nc-contract-a9eb-20261001`; none were published.
The following immutable Docker image IDs identify the tested artifacts.

| Tag suffix | Image ID |
|---|---|
| `dummy` | `sha256:6d2899f189e7693b6e657a81b42d93cab57cdefbbbc9f9190ed806a808a896f2` |
| `claude` | `sha256:b8856cd89cee54bc532e30d537a134bbef27e26ccfb89e4053e2c321f5479e4e` |
| `codex` | `sha256:f3bda72d9606cefa285d19bb649e172469b8c6def222bf511af1ddee8d1d6c35` |
| `deepagents` | `sha256:ce6584cb96b309135363cc408e2ced98f8e8db280bc432bd30a7eb292254dc53` |
| `hermes` | `sha256:9bf050c1b0bf50a09c5f20c57f3b17ef79c3e5d6672d2420e084a4b7dcb2aa7c` |
| `mini-swe-agent` | `sha256:42c82a469e41f3f99c648ea0c7e0d1f38281be1fadac13e34a3b5b0229cd4d29` |
| `nooa` | `sha256:1b018b53889426412eea9d78348b1dee1f94042237fe9362f7ad160920a2a285` |
| `nooa-bench` | `sha256:b08efdd9daeaf7603433d7008a11e7d0cc773ce145fe367b8bb7f6f6deb89da4` |
| `openclaw` | `sha256:15c5fa5d3b7a5b4f171de73ff72a11c621752d3932ed3c2c95fce4fcf559d999` |
| `pi` | `sha256:1f1ac5ce955e5b5b5c11731b93708ac9e592f1c131d94eb8e6cf1e5adb799e8c` |
| `remote-agent` | `sha256:95d941ae2692f38dbc0007fc8118ed0e818af66f91b695276d51de935361a16b` |

The image sources match NemoClaw implementation commit `38fc6e7c3cad08239af7a47342f2e1b19097e509`.
The builds used the working changes based on `18ed1a86161fa67d3503ad99ace5bee8cdb876b9`.
Those image sources were unchanged when integrated with `2454aa551d8d2766e5d19911e7dc871c4c13b4a9`.
Production images use Fabric `24f068c895e5cbc30286bc743498be4e5014d658` with the repository's error-code patch.

Dummy, Claude, Codex, and Deep Agents used the current Dockerfile build.
Host disk capacity prevented a complete rebuild of the remaining seven native dependency layers.
Those images reused existing native layers after checking their harness identity, Fabric source pin, and Python version; OpenClaw's retained reconfiguration patch and implementation were also checked.
The rollout installed the current shared host and backend, plus the newly built patched Fabric runtime from the Deep Agents image, then regenerated the capability and discovery labels.
Each of those seven images retains its base-image and runtime-image references, dated modifications, previous provenance, and overlay build source under `/opt/nemoclaw`.
The installed-image tests verified the resulting source and requirement hashes.
This result is not evidence of a complete rebuild of those seven images from current upstream dependencies.

## Results

| Check | Observed result |
|---|---|
| Shared command suite | All six tests passed in dummy; four passed and two dummy-only tests were skipped in each real image. |
| Installed metadata and retained-source suite | Passed for all ten real images; adapter-specific inapplicable cases were skipped. |
| Generic image behavior | 69 tests passed against the installed Fabric fixture and reference backend. |
| Independent reference backend | Ten tests passed without Fabric installed. |
| Target selection, Dockerfile checks, Ruff, and proxy fixtures | Passed. |
| Native OpenClaw, Hermes, and Pi fixtures | Each completed two native invocations against local simulated inference. |
| OpenClaw reconfiguration | Two tests passed, including retained state across host replacement. |
| Hermes authentication and owned shutdown | Passed without model inference. |

Successful configuration, generation conflicts, invocation, and readiness-failure scenarios in the shared command suite ran only in dummy.
The separate native invocation fixtures call Fabric's runtime API directly; they do not independently qualify `fabric-agent invoke` for those adapters.

The shared command qualifier ran in owned containers with networking disabled, a read-only root filesystem, and temporary sandbox storage.
Native fixtures used separate disposable containers with networking disabled and no deployment credentials.
No existing deployment was reconfigured or stopped.
The Hermes fixture emitted its native SQLite warning and selected DELETE journaling; this run did not change that dependency.

Real Fabric backends still advertise no native health checks and return unsupported health, which fails apply while preserving resources.
Dummy health validates the reference behavior only.
The dummy has no Fabric discovery catalog and is not an SDK deployment harness.
The SDK's migration to selected-image validation and contract-version compatibility remains separate NemoClaw work.
AMD64 execution and OpenShell sandbox-stop guarantees were not tested here.
See [upstream ownership](../design/fabric-management.md#upstream-ownership) for the remaining coordination.
