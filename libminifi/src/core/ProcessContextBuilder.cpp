/**
 * @file ProcessGroup.cpp
 * ProcessGroup class implementation
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
#include "core/ProcessContextBuilder.h"
#include <time.h>
#include <vector>
#include <memory>
#include <string>
#include <queue>
#include <map>
#include <set>
#include <chrono>
#include <thread>
#include "core/Processor.h"
#include "core/logging/LoggerConfiguration.h"

namespace org {
namespace apache {
namespace nifi {
namespace minifi {
namespace core {

ProcessContextBuilder::ProcessContextBuilder(const std::string &name, minifi::utils::Identifier &uuid)
    : core::CoreComponent(name, uuid) {
  content_repo_ = org::apache::nifi::minifi::utils::debug_make_shared<core::repository::FileSystemRepository>();
  configuration_ = org::apache::nifi::minifi::utils::debug_make_shared<minifi::Configure>();
}

ProcessContextBuilder::ProcessContextBuilder(const std::string &name)
    : core::CoreComponent(name) {
  content_repo_ = org::apache::nifi::minifi::utils::debug_make_shared<core::repository::FileSystemRepository>();
  configuration_ = org::apache::nifi::minifi::utils::debug_make_shared<minifi::Configure>();
}

org::apache::nifi::minifi::utils::debug_shared_ptr<ProcessContextBuilder> ProcessContextBuilder::withProvider(core::controller::ControllerServiceProvider* controller_service_provider) {
  controller_service_provider_ = controller_service_provider;
  return this->shared_from_this();
}

org::apache::nifi::minifi::utils::debug_shared_ptr<ProcessContextBuilder> ProcessContextBuilder::withProvenanceRepository(const org::apache::nifi::minifi::utils::debug_shared_ptr<core::Repository> &repo) {
  prov_repo_ = repo;
  return this->shared_from_this();
}

org::apache::nifi::minifi::utils::debug_shared_ptr<ProcessContextBuilder> ProcessContextBuilder::withFlowFileRepository(const org::apache::nifi::minifi::utils::debug_shared_ptr<core::Repository> &repo) {
  flow_repo_ = repo;
  return this->shared_from_this();
}

org::apache::nifi::minifi::utils::debug_shared_ptr<ProcessContextBuilder> ProcessContextBuilder::withContentRepository(const org::apache::nifi::minifi::utils::debug_shared_ptr<core::ContentRepository> &repo) {
  content_repo_ = repo;
  return this->shared_from_this();
}

org::apache::nifi::minifi::utils::debug_shared_ptr<ProcessContextBuilder> ProcessContextBuilder::withConfiguration(const org::apache::nifi::minifi::utils::debug_shared_ptr<minifi::Configure> &configuration) {
  configuration_ = configuration;
  return this->shared_from_this();
}

org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessContext> ProcessContextBuilder::build(const org::apache::nifi::minifi::utils::debug_shared_ptr<ProcessorNode> &processor) {
  return org::apache::nifi::minifi::utils::debug_make_shared<core::ProcessContext>(processor, controller_service_provider_, prov_repo_, flow_repo_, configuration_, content_repo_);
}

} /* namespace core */
} /* namespace minifi */
} /* namespace nifi */
} /* namespace apache */
} /* namespace org */
