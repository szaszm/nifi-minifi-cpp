# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements.  See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership.  The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License.  You may obtain a copy of the License at
#
#   https://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied.  See the License for the
# specific language governing permissions and limitations
# under the License.

import os
import urllib.request
from typing import List

from minifi_behave.containers.docker_image_builder import DockerImageBuilder
from minifi_behave.core.hooks import common_after_scenario
from minifi_behave.core.hooks import common_before_scenario, get_minifi_container_image
from minifi_behave.core.minifi_test_context import MinifiTestContext

import ssl
ssl._create_default_https_context = ssl._create_unverified_context

# Model / label / image assets fetched on first use. All hosted on
# public buckets or the sonos/tract repo; kept out of the git history so
# clones stay small.
REMOTE_ASSETS: dict[str, str] = {
    # ImageNet MobileNetV2 classifier (~14 MB) — the reference model used by
    # tract's own onnx-mobilenet-v2 example.
    "mobilenetv2-7.onnx":
        "https://s3.amazonaws.com/tract-ci-builds/tests/mobilenetv2-7.onnx",
    # 1000-class ImageNet labels (line N = class N; line 0 is "dummy" so
    # 1-based class ids map to the label directly).
    "imagenet_slim_labels.txt":
        "https://raw.githubusercontent.com/sonos/tract/main/examples/"
        "onnx-mobilenet-v2/imagenet_slim_labels.txt",
    # Same test image tract's example uses. MobileNetV2 confidently
    # classifies this as "military uniform".
    "grace_hopper.jpg":
        "https://raw.githubusercontent.com/sonos/tract/main/examples/"
        "onnx-mobilenet-v2/grace_hopper.jpg",
    # UltraFace RFB-320 (~1.2 MB): 2-output SSD-style detector matching the
    # existing FilterBoundingBoxes defaults (Xyxy boxes, class 0 = background,
    # softmax over 2 classes: background/face). 320x240 RGB, mean=127, std=128.
    "version-RFB-320.onnx":
        "https://github.com/onnx/models/raw/refs/heads/main/validated/vision/"
        "body_analysis/ultraface/models/version-RFB-320.onnx",
}


def _ensure_asset(cache_dir: str, filename: str) -> str:
    """Download `filename` from REMOTE_ASSETS into `cache_dir` if absent.

    Returns the absolute path on disk. Downloads are atomic-ish — we write to
    a `.part` sibling and rename on success so an interrupted run can't leave
    a truncated file behind that later tests mistake for a valid asset.
    """
    dest = os.path.join(cache_dir, filename)
    if os.path.exists(dest):
        return dest
    url = REMOTE_ASSETS[filename]
    os.makedirs(cache_dir, exist_ok=True)
    tmp = dest + ".part"
    print(f"[minifi_tensor tests] fetching {filename} from {url}")
    urllib.request.urlretrieve(url, tmp)
    os.replace(tmp, dest)
    return dest


def add_extension_to_minifi_container(
    extension_name: str, possible_paths: List[str], context: MinifiTestContext
):
    new_container_name = f"apacheminificpp:{extension_name}"
    is_windows = os.name == "nt"
    if is_windows:
        lib_filename = f"{extension_name}.dll"
        container_extension_dir = (
            "C:/Program Files/ApacheNiFiMiNiFi/nifi-minifi-cpp/extensions"
        )
    else:
        lib_filename = f"lib{extension_name}.so"
        container_extension_dir = "/opt/minifi/minifi-current/extensions/"

    host_path = None
    for path in possible_paths:
        if os.path.exists(os.path.join(path, lib_filename)):
            host_path = os.path.join(path, lib_filename)
            break

    assert host_path is not None, (
        f"Could not find {lib_filename} in {[p for p in possible_paths]}"
    )

    with open(host_path, "rb") as f:
        lib_content = f.read()

    base_img = get_minifi_container_image()

    if is_windows:
        dockerfile = f"""
FROM {base_img}
COPY ["{lib_filename}", "{container_extension_dir}/{lib_filename}"]
"""
    else:
        dockerfile = f"""
FROM {base_img}
COPY --chown=minificpp:minificpp {lib_filename} {container_extension_dir}
RUN chmod 755 {container_extension_dir}{lib_filename}
"""

    builder = DockerImageBuilder(
        image_tag=new_container_name,
        dockerfile_content=dockerfile,
        files_on_context={lib_filename: lib_content},
    )

    builder.build()
    return new_container_name


def before_all(context):
    dir_path = os.path.dirname(os.path.realpath(__file__))
    build_path = os.path.normpath(os.path.join(dir_path, "../../../target/release/"))
    deps_build_path = os.path.normpath(
        os.path.join(dir_path, "../../../target/release/deps/")
    )
    add_extension_to_minifi_container(
        "minifi_tensor", [build_path, deps_build_path], context
    )

    # Assets live under features/resources/ so `context.resource_dir` (used by
    # the "host resource file ... is bound to ..." step) picks them up. Cached
    # across scenarios so we only pay the download once per checkout.
    context.tensor_resource_dir = os.path.join(dir_path, "resources")
    os.makedirs(context.tensor_resource_dir, exist_ok=True)
    for name in REMOTE_ASSETS:
        _ensure_asset(context.tensor_resource_dir, name)


def before_scenario(context, scenario):
    context.minifi_container_image = "apacheminificpp:minifi_tensor"
    common_before_scenario(context, scenario)
    context.resource_dir = context.tensor_resource_dir


def after_scenario(context, scenario):
    common_after_scenario(context, scenario)
