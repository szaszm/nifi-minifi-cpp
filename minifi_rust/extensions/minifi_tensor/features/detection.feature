# Licensed to the Apache Software Foundation (ASF) under one or more
# contributor license agreements.  See the NOTICE file distributed with
# this work for additional information regarding copyright ownership.
# The ASF licenses this file to You under the Apache License, Version 2.0
# (the "License"); you may not use this file except in compliance with
# the License.  You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

@SUPPORTS_WINDOWS
Feature: End-to-end face detection with UltraFace (SSD)

  Scenario: Grace Hopper image yields at least one face detection
    Given a host resource file "grace_hopper.jpg" is copied to the "/tmp/input/grace_hopper.jpg" path in the MiNiFi container
    And a host resource file "version-RFB-320.onnx" is copied to the "/tmp/models/ultraface.onnx" path in the MiNiFi container

    And a TractModelService controller service named "UltraFace" is set up and the "Model File Path" property set to "/tmp/models/ultraface.onnx"
    And the "Model format" property of the UltraFace controller service is set to "Onnx"

    And a GetFile processor with the "Input Directory" property set to "/tmp/input"
    And the "Keep Source File" property of the GetFile processor is set to "false"

    # UltraFace RFB-320 expects 320x240 (WxH) RGB with mean=127, std=128 applied
    # per channel. Aspect-preserving letterbox keeps Grace Hopper's face
    # geometry intact instead of stretching it.
    And an ImageToTensor processor with the "Target width" property set to "320"
    And the "Target height" property of the ImageToTensor processor is set to "240"
    And the "Resize filter" property of the ImageToTensor processor is set to "Bilinear"
    And the "Resize mode" property of the ImageToTensor processor is set to "Letterbox"
    And the "Color format" property of the ImageToTensor processor is set to "RGB"
    And the "Tensor shape format" property of the ImageToTensor processor is set to "CHW"
    And the "Mean" property of the ImageToTensor processor is set to "127.0"
    And the "Standard Deviation" property of the ImageToTensor processor is set to "128.0"

    And an InvokeTractModel processor with the "Tract model service" property set to "UltraFace"

    # UltraFace: output 0 = scores [1, N, 2] (softmax over background/face),
    # output 1 = boxes [1, N, 4] in Xyxy normalised to 0..1. Class 0 is
    # background so leave the defaults.
    And a FilterBoundingBoxes processor with the "Confidence Threshold" property set to "0.5"
    And the "IoU Threshold" property of the FilterBoundingBoxes processor is set to "0.45"
    And the "Score output index" property of the FilterBoundingBoxes processor is set to "0"
    And the "Box output index" property of the FilterBoundingBoxes processor is set to "1"
    And the "Box format" property of the FilterBoundingBoxes processor is set to "Xyxy"
    And the "Score activation" property of the FilterBoundingBoxes processor is set to "Softmax"
    And the "Background class index" property of the FilterBoundingBoxes processor is set to "0"

    And a PutFile processor with the "Directory" property set to "/tmp/output"

    And a LogAttribute processor with the "FlowFiles To Log" property set to "0"
    And LogAttribute is EVENT_DRIVEN

    And the "success" relationship of the GetFile processor is connected to the ImageToTensor
    And the "success" relationship of the ImageToTensor processor is connected to the InvokeTractModel
    And the "success" relationship of the InvokeTractModel processor is connected to the FilterBoundingBoxes
    And the "success" relationship of the FilterBoundingBoxes processor is connected to the LogAttribute
    And the "success" relationship of the LogAttribute processor is connected to the PutFile
    And ImageToTensor's failure relationship is auto-terminated
    And InvokeTractModel's failure relationship is auto-terminated
    And FilterBoundingBoxes's failure relationship is auto-terminated
    And PutFile's success relationship is auto-terminated
    And PutFile's failure relationship is auto-terminated

    When the MiNiFi instance starts up

    Then the Minifi logs match the following regex: "key:object.count value:[1-9][0-9]*" in less than 60 seconds
    And the Minifi logs contain the following message: "key:mime.type value:application/json" in less than 1 seconds
    And at least one file in "/tmp/output" content match the following regex: "\"class_id\":1" in less than 30 seconds
    And at least one file in "/tmp/output" content match the following regex: "\"confidence\":0\.[5-9][0-9]*" in less than 30 seconds
    And the Minifi logs do not contain errors
