/**
 * @file AzureStorageProcessorBase.h
 * AzureStorageProcessorBase class declaration
 *
 * Licensed to the Apache Software Foundation (ASF) under one or more
 * contributor license agreements.  See the NOTICE file distributed with
 * this work for additional information regarding copyright ownership.
 * The ASF licenses this file to You under the Apache License, Version 2.0
 * (the "License"); you may not use this file except in compliance with
 * the License.  You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#pragma once

#include <memory>
#include <string>
#include <optional>
#include <utility>
#include <tuple>

#include "core/PropertyDefinition.h"
#include "core/Property.h"
#include "core/PropertyDefinitionBuilder.h"
#include "core/Processor.h"
#include "core/logging/Logger.h"
#include "storage/AzureStorageCredentials.h"

namespace org::apache::nifi::minifi::azure::processors {

class AzureStorageProcessorBase : public core::ProcessorImpl {
 public:
  EXTENSIONAPI static constexpr auto AzureStorageCredentialsService = core::PropertyDefinitionBuilder<>::createProperty("Azure Storage Credentials Service")
      .withDescription("Name of the Azure Storage Credentials Service used to retrieve the connection string from.")
      .build();
  EXTENSIONAPI static constexpr auto Properties = std::to_array<core::PropertyReference>({AzureStorageCredentialsService});

  AzureStorageProcessorBase(std::string_view name, const minifi::utils::Identifier& uuid, const std::shared_ptr<core::logging::Logger>& logger)
    : core::ProcessorImpl(name, uuid),
      logger_(logger) {
  }

 protected:
  enum class GetCredentialsFromControllerResult {
    OK,
    CONTROLLER_NAME_EMPTY,
    CONTROLLER_NAME_INVALID
  };

  std::tuple<GetCredentialsFromControllerResult, std::optional<storage::AzureStorageCredentials>> getCredentialsFromControllerService(core::ProcessContext &context) const;

  std::shared_ptr<core::logging::Logger> logger_;
};

}  // namespace org::apache::nifi::minifi::azure::processors
