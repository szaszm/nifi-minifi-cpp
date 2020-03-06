/**
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
#ifndef LIBMINIFI_INCLUDE_CORE_REPOSITORY_DatabaseContentRepository_H_
#define LIBMINIFI_INCLUDE_CORE_REPOSITORY_DatabaseContentRepository_H_

#include "rocksdb/db.h"
#include "rocksdb/merge_operator.h"
#include "core/Core.h"
#include "core/Connectable.h"
#include "core/ContentRepository.h"
#include "properties/Configure.h"
#include "core/logging/LoggerConfiguration.h"

namespace org {
namespace apache {
namespace nifi {
namespace minifi {
namespace core {
namespace repository {

class StringAppender : public rocksdb::AssociativeMergeOperator {
 public:
  // Constructor: specify delimiter
  explicit StringAppender() = default;

  bool Merge(const rocksdb::Slice& key, const rocksdb::Slice* existing_value, const rocksdb::Slice& value, std::string* new_value, rocksdb::Logger* logger) const override {
    // Clear the *new_value for writing.
    if (nullptr == new_value) {
      return false;
    }
    new_value->clear();

    if (!existing_value) {
      // No existing_value. Set *new_value = value
      new_value->assign(value.data(), value.size());
    } else {
      new_value->reserve(existing_value->size() + value.size());
      new_value->assign(existing_value->data(), existing_value->size());
      new_value->append(value.data(), value.size());
    }

    return true;
  }

  const char* Name() const override {
    return "StringAppender";
  }
};

/**
 * DatabaseContentRepository is a content repository that stores data onto the local file system.
 */
class DatabaseContentRepository final : public core::ContentRepository, public core::Connectable {
 public:

  explicit DatabaseContentRepository(const std::string& name = getClassName<DatabaseContentRepository>(), const utils::Identifier& uuid = utils::Identifier())
      : core::Connectable(name, uuid) {
  }

  ~DatabaseContentRepository() final {
    stop();
  }

  bool initialize(const std::shared_ptr<minifi::Configure> &configuration) final;

  void stop() final;

  std::shared_ptr<io::BaseStream> write(const std::shared_ptr<minifi::ResourceClaim> &claim, bool append = false) final;

  std::shared_ptr<io::BaseStream> read(const std::shared_ptr<minifi::ResourceClaim> &claim) final;

  bool close(const std::shared_ptr<minifi::ResourceClaim> &claim) final {
    return remove(claim);
  }

  bool remove(const std::shared_ptr<minifi::ResourceClaim> &claim) final;

  bool exists(const std::shared_ptr<minifi::ResourceClaim> &streamId) final;

  void yield() final { }

  /**
   * Determines if we are connected and operating
   */
  bool isRunning() final {
    return true;
  }

  /**
   * Determines if work is available by this connectable
   * @return boolean if work is available.
   */
  bool isWorkAvailable() final {
    return true;
  }

 private:
  bool is_valid_{ false };
  rocksdb::DB* db_{ nullptr };
  std::shared_ptr<logging::Logger> logger_{ logging::LoggerFactory<DatabaseContentRepository>::getLogger() };
};

} /* namespace repository */
} /* namespace core */
} /* namespace minifi */
} /* namespace nifi */
} /* namespace apache */
} /* namespace org */

#endif /* LIBMINIFI_INCLUDE_CORE_REPOSITORY_DatabaseContentRepository_H_ */
