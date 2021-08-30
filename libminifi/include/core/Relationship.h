/**
 * @file Relationship.h
 * Relationship class declaration
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
#ifndef LIBMINIFI_INCLUDE_CORE_RELATIONSHIP_H_
#define LIBMINIFI_INCLUDE_CORE_RELATIONSHIP_H_

#include <string>
#include <utility>

namespace org::apache::nifi::minifi::core {

class Relationship {
 public:
  Relationship() = default;

  Relationship(std::string name, std::string description)
      : name_(std::move(name)),
        description_(std::move(description)) {
  }

  Relationship(const Relationship& other) = default;
  Relationship &operator=(const Relationship& other) = default;
  ~Relationship() = default;

  [[nodiscard]] std::string getName() const {
    return name_;
  }

  [[nodiscard]] std::string getDescription() const {
    return description_;
  }

  bool operator <(const Relationship & right) const {
    return name_ < right.name_;
  }

  bool operator==(const Relationship &other) const {
    return name_ == other.name_;
  }

 protected:
  std::string name_ = "undefined";
  std::string description_;
};

}  // namespace org::apache::nifi::minifi::core

#endif  // LIBMINIFI_INCLUDE_CORE_RELATIONSHIP_H_
