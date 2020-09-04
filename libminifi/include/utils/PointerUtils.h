/**
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

#include <boost/stacktrace.hpp>

#include <memory>
#include <utility>
#include <type_traits>
#include <sstream>
#include <unordered_map>
#include <map>
#include <mutex>

#include "utils/GeneralUtils.h"

namespace org {
namespace apache {
namespace nifi {
namespace minifi {
namespace utils {

namespace detail {
struct make_shared_t{};
struct pointer_cast_t{};
struct shared_from_this_t{};

inline std::unordered_map<std::string /* typeid(T).name() */, std::unordered_map<const void* /* obj_ptr */, std::map<const void* /* shared_ptr_this */, std::string /* stack trace */>>>& get_debug_map() {
  static std::unordered_map<std::string /* typeid(T).name() */, std::unordered_map<const void* /* obj_ptr */, std::map<const void* /* shared_ptr_this */, std::string /* stack trace */>>> collection;
  return collection;
}

inline std::unique_lock<std::mutex> lock_debug_map() {
  static std::mutex mutex;
  return std::unique_lock<std::mutex>{mutex};
}

struct managed_object_metadata {
  const void* ptr = nullptr;
  std::string type_id;

  void add() {
    if (!ptr) return;
    const auto l = lock_debug_map();
    std::ostringstream trace;
    trace << boost::stacktrace::stacktrace{};
    get_debug_map()[this->type_id][this->ptr][this] = trace.str();
  }

  template<typename T>
  void check(const std::shared_ptr<T>& p) const {
    return;
    gsl_Expects((p == nullptr) == (this->ptr == nullptr));
    if(!ptr) return;
    const auto l = lock_debug_map();
    const auto& sptrs = detail::get_debug_map()[type_id][this->ptr];
    const auto usecnt = p.use_count();
    gsl_Expects(sptrs.size() == gsl::narrow<size_t>(usecnt));
  }

  void remove() {
    if (!ptr) return;
    const auto l = lock_debug_map();
    auto& map = detail::get_debug_map()[type_id][ptr];
    const auto removed = map.erase(this);
    gsl_Expects(removed > 0);
  }

  void reset() {
    if(!ptr) return;
    remove();
    ptr = nullptr;
    type_id.clear();
  }

  managed_object_metadata() = default;

  managed_object_metadata(const void* ptr, std::string type_id)
      :ptr{ptr}, type_id{std::move(type_id)} {
    add();
  }

  managed_object_metadata(const managed_object_metadata& other)
      :ptr{other.ptr}, type_id{other.type_id} {
    add();
  }

  managed_object_metadata(managed_object_metadata&& other) noexcept
      :ptr{other.ptr}, type_id{other.type_id} {
    other.reset();
  }

  managed_object_metadata& operator=(const managed_object_metadata& other) {
    if(&other == this) return *this;
    remove();
    ptr = other.ptr;
    type_id = other.type_id;
    add();
    return *this;
  }

  managed_object_metadata& operator=(managed_object_metadata&& other) noexcept {
    if(&other == this) return *this;
    remove();
    ptr = other.ptr;
    type_id = other.type_id;
    add();
    other.reset();
    return *this;
  }

  ~managed_object_metadata() {
    remove();
  }
};
} /* namespace detail */


template<typename>
struct debug_shared_ptr;

template<typename T>
struct debug_shared_ptr : private std::shared_ptr<T> {
  debug_shared_ptr() = default;

 private:
  explicit debug_shared_ptr(std::shared_ptr<T> sptr)
    :std::shared_ptr<T>{std::move(sptr)} {
    metadata.check(*this);
  }

  debug_shared_ptr(detail::make_shared_t, std::shared_ptr<T> sptr)
    :debug_shared_ptr{std::move(sptr)} {
    metadata.check(*this);
  }

  template<typename U, typename... Args>
  friend debug_shared_ptr<U> debug_make_shared(Args&&...);

  debug_shared_ptr(detail::pointer_cast_t, const void* obj_ptr, std::string type_id, std::shared_ptr<T> sptr)
    :std::shared_ptr<T>{std::move(sptr)}, metadata{obj_ptr, std::move(type_id)} {
  }
  template<typename TT, typename U>
  friend debug_shared_ptr<TT> dynamic_pointer_cast(debug_shared_ptr<U>);
  template<typename TT, typename U>
  friend debug_shared_ptr<TT> static_pointer_cast(debug_shared_ptr<U>);
  template<typename TT, typename U>
  friend debug_shared_ptr<TT> const_pointer_cast(debug_shared_ptr<U>);

  template<typename U>
  friend struct debug_shared_ptr;
 public:
  explicit debug_shared_ptr(T* ptr)
    :std::shared_ptr<T>{ptr}, metadata{ptr, typeid(ptr).name()}
  { }

  debug_shared_ptr(detail::shared_from_this_t, std::shared_ptr<T> sptr)
      :std::shared_ptr<T>{std::move(sptr)}
  { }

  debug_shared_ptr(const debug_shared_ptr& other)
    :std::shared_ptr<T>{other}, metadata{other.metadata}
  { }

  debug_shared_ptr(debug_shared_ptr&& other) noexcept
      :std::shared_ptr<T>{std::move(other)}, metadata{other.metadata}
  { }

  debug_shared_ptr& operator=(const debug_shared_ptr& other) {
    if(&other == this) return *this;
    this->std::shared_ptr<T>::operator=(other);
    this->metadata = other.metadata;
    metadata.check(*this);
    other.metadata.check(other);
    return *this;
  }

  template<typename U, typename = typename std::enable_if<std::is_convertible<U*, T*>::value>::type>
  debug_shared_ptr& operator=(const debug_shared_ptr<U>& other) {
    if(reinterpret_cast<const debug_shared_ptr<T>*>(&other) == this) return *this;
    this->std::shared_ptr<T>::operator=(other);
    this->metadata = other.metadata;
    metadata.check(*this);
    other.metadata.check(other);
    return *this;
  }

  debug_shared_ptr& operator=(debug_shared_ptr&& other) noexcept {
    if(&other == this) return *this;
    this->std::shared_ptr<T>::operator=(std::move(other));
    this->metadata = std::move(other.metadata);
    metadata.check(*this);
    other.metadata.check(other);
    return *this;
  }

  template<typename U, typename = typename std::enable_if<std::is_convertible<U*, T*>::value>::type>
  debug_shared_ptr& operator=(debug_shared_ptr<U>&& other) noexcept {
    if(reinterpret_cast<const debug_shared_ptr<T>*>(&other) == this) return *this;
    this->std::shared_ptr<T>::operator=(std::move(other));
    this->metadata = std::move(other.metadata);
    metadata.check(*this);
    other.metadata.check(other);
    return *this;
  }

  debug_shared_ptr& operator=(std::unique_ptr<T>&& ptr) {
    this->std::shared_ptr<T>::operator=(std::move(ptr));
    this->metadata = detail::managed_object_metadata{get(), typeid(get()).name()};
    metadata.check(*this);
    return *this;
  }

  debug_shared_ptr& operator=(std::nullptr_t) noexcept {
    this->std::shared_ptr<T>::operator=(nullptr);
    this->metadata.reset();
    return *this;
  }

  template<typename U, typename = typename std::enable_if<std::is_convertible<U*, T*>::value>::type>
  debug_shared_ptr(const debug_shared_ptr<U>& other)
      :std::shared_ptr<T>{other}, metadata{other.metadata}
  {
    metadata.check(*this);
  }

  debug_shared_ptr(std::nullptr_t)
      :std::shared_ptr<T>{}, metadata{}
  {}

  debug_shared_ptr(std::unique_ptr<T>&& ptr)
    :debug_shared_ptr{ptr.release()}
  {
    metadata.check(*this);
  }

  ~debug_shared_ptr() {
    auto l = detail::lock_debug_map();
    metadata.check(*this);
  }

  std::shared_ptr<T> sptr() const noexcept { return *this; }
  using std::shared_ptr<T>::operator bool;
  using std::shared_ptr<T>::operator*;
  using std::shared_ptr<T>::operator->;
  using std::shared_ptr<T>::get;
  using std::shared_ptr<T>::use_count;
  using std::shared_ptr<T>::unique;

  bool operator==(std::nullptr_t) const noexcept { return !get(); }
  bool operator!=(std::nullptr_t) const noexcept { return get(); }
  bool operator==(const debug_shared_ptr<T>& other) const noexcept { return get() == other.get(); }
  bool operator!=(const debug_shared_ptr<T>& other) const noexcept { return get() != other.get(); }
  template<typename U, typename = typename std::enable_if<std::is_convertible<U*, T*>::value>::type>
  bool operator==(const debug_shared_ptr<U>& other) const noexcept { return get() == other.get(); }
  template<typename U, typename = typename std::enable_if<std::is_convertible<U*, T*>::value>::type>
  bool operator!=(const debug_shared_ptr<U>& other) const noexcept { return get() != other.get(); }
  bool operator<(const debug_shared_ptr<T>& other) const noexcept { return get() < other.get(); }
  bool operator>(const debug_shared_ptr<T>& other) const noexcept { return get() > other.get(); }
  bool operator<=(const debug_shared_ptr<T>& other) const noexcept { return get() <= other.get(); }
  bool operator>=(const debug_shared_ptr<T>& other) const noexcept { return get() >= other.get(); }

  void reset() { *this = nullptr; metadata.reset(); }
  void reset(T* ptr) { *this = debug_shared_ptr{ptr}; }

 private:
  detail::managed_object_metadata metadata{get(), typeid(get()).name()};
};

template<typename T>
bool operator==(std::nullptr_t, const debug_shared_ptr<T>& p) noexcept { return p == nullptr; }
template<typename T>
bool operator!=(std::nullptr_t, const debug_shared_ptr<T>& p) noexcept { return p != nullptr; }
template<typename T, typename U, typename = typename std::enable_if<std::is_convertible<U*, T*>::value>::type>
bool operator==(const debug_shared_ptr<U>& p1, const debug_shared_ptr<T>& p2) noexcept { return p2 == p1; }
template<typename T, typename U, typename = typename std::enable_if<std::is_convertible<U*, T*>::value>::type>
bool operator!=(const debug_shared_ptr<U>& p1, const debug_shared_ptr<T>& p2) noexcept { return p2 != p1; }

template<typename T>
std::ostream& operator<<(std::ostream& os, const debug_shared_ptr<T>& ptr) { return os << ptr.sptr(); }

template<typename T, typename... Args>
debug_shared_ptr<T> debug_make_shared(Args&&... args) {
  return debug_shared_ptr<T>(detail::make_shared_t{}, std::make_shared<T>(std::forward<Args>(args)...));
}

template<typename T>
struct enable_debug_shared_from_this : std::enable_shared_from_this<T> {
  using std::enable_shared_from_this<T>::enable_shared_from_this;
  debug_shared_ptr<T> shared_from_this() {
    return debug_shared_ptr<T>{detail::shared_from_this_t{}, this->std::enable_shared_from_this<T>::shared_from_this()};
  }
  debug_shared_ptr<T const> shared_from_this() const {
    return debug_shared_ptr<T const>{detail::shared_from_this_t{}, this->std::enable_shared_from_this<T>::shared_from_this()};
  }
};

template<typename T, typename U>
debug_shared_ptr<T> dynamic_pointer_cast(debug_shared_ptr<U> ptr) {
  return debug_shared_ptr<T>{detail::pointer_cast_t{}, ptr.metadata.ptr, ptr.metadata.type_id, std::dynamic_pointer_cast<T>(ptr)};
}

template<typename T, typename U>
debug_shared_ptr<T> static_pointer_cast(debug_shared_ptr<U> ptr) {
  return debug_shared_ptr<T>{detail::pointer_cast_t{}, ptr.metadata.ptr, ptr.metadata.type_id, std::static_pointer_cast<T>(ptr)};
}

template<typename T, typename U>
debug_shared_ptr<T> const_pointer_cast(debug_shared_ptr<U> ptr) {
  return debug_shared_ptr<T>{detail::pointer_cast_t{}, ptr.metadata.ptr, ptr.metadata.type_id, std::const_pointer_cast<T>(ptr)};
}

} /* namespace utils */
} /* namespace minifi */
} /* namespace nifi */
} /* namespace apache */
} /* namespace org */

using org::apache::nifi::minifi::utils::dynamic_pointer_cast;
using org::apache::nifi::minifi::utils::static_pointer_cast;
using org::apache::nifi::minifi::utils::const_pointer_cast;

namespace std {
template<typename T>
struct hash<org::apache::nifi::minifi::utils::debug_shared_ptr<T>> {
  size_t operator()(const org::apache::nifi::minifi::utils::debug_shared_ptr<T>& p) const noexcept { return hash<T*>{}(p.get()); }
};
}  // namespace std
