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

#include "utils/MinifiConcurrentQueue.h"
#include "utils/gsl.h"

#include <functional>
#include <future>
#include <thread>

namespace org { namespace apache { namespace nifi { namespace minifi { namespace extensions { namespace systemd {

class WorkerThread final {
 public:
  WorkerThread() :thread_{&WorkerThread::run, this} {}

  ~WorkerThread() {
    work_.stop();
    thread_.join();
  }

  template<typename... Args>
  void enqueue(Args&&... args) { work_.enqueue(std::forward<Args>(args)...); }

 private:
  void run() noexcept {
    while (work_.isRunning()) {
      work_.consumeWait([](std::function<void() noexcept>& f) { f(); });
    }
  }
  std::thread thread_;
  utils::ConditionConcurrentQueue<std::function<void() noexcept>> work_;
};

/**
 * A worker that executes arbitrary functions with no parameters asynchronously on an internal thread, returning a future to the result.
 */
class Worker final {
 public:
  template<typename Func>
  auto enqueue(Func func) -> std::future<decltype(func())> {
    using result_type = decltype(func());
    std::packaged_task<result_type()> task{std::move(func)};
    auto future = task.get_future();
    worker_thread_.enqueue(std::move(task));
    return future;
  }
 private:
  WorkerThread worker_thread_;
};

}}}}}}  // namespace org::apache::nifi::minifi::extensions::systemd
