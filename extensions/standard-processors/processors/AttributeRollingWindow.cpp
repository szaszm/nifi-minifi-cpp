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
#include "AttributeRollingWindow.h"
#include <algorithm>
#include <numeric>
#include "fmt/format.h"
#include "core/ProcessContext.h"
#include "core/ProcessSession.h"
#include "core/Resource.h"
#include "utils/expected.h"
#include "utils/OptionalUtils.h"

namespace org::apache::nifi::minifi::processors {

void AttributeRollingWindow::onSchedule(core::ProcessContext* context, core::ProcessSessionFactory*) {
  gsl_Expects(context);
  time_window_ = context->getProperty<core::TimePeriodValue>(TimeWindow)
      | utils::transform(&core::TimePeriodValue::getMilliseconds);
  window_length_ = context->getProperty<size_t>(WindowLength)
      | utils::filter([](size_t value) { return value > 0; });
  if (!time_window_ && !window_length_) {
    throw minifi::Exception{ExceptionType::PROCESS_SCHEDULE_EXCEPTION, "Either 'Time window' or 'Window length' must be set"};
  }
  gsl_Ensures(runningInvariant());
}

void AttributeRollingWindow::onTrigger(core::ProcessContext* context, core::ProcessSession* session) {
  gsl_Expects(context && session && runningInvariant());
  const auto flow_file = session->get();
  if (!flow_file) { yield(); return; }
  gsl_Assert(flow_file);
  const auto current_value_opt = context->getProperty(ValueToTrack, flow_file);
  if (!current_value_opt) {
    logger_->log_warn("Missing value to track, flow file uuid: {}", flow_file->getUUIDStr());
    session->transfer(flow_file, Failure);
    return;
  }
  const auto current_value = [&current_value_opt] {
    try {
      return std::stod(*current_value_opt);
    } catch (const std::exception& ex) {
      throw minifi::Exception{ExceptionType::PROCESSOR_EXCEPTION,
          fmt::format("Failed to convert 'Value to track' of '{}' to double", *current_value_opt)};
    }
  }();
  // copy: so we can release the lock sooner
  const auto state_copy = [&, now = std::chrono::system_clock::now()] {
    std::lock_guard lg{state_mutex_};
    state_.add(now, current_value);
    if (window_length_) {
      state_.shrinkToSize(*window_length_);
    } else {
      gsl_Assert(time_window_);
      state_.removeOlderThan(now - *time_window_);
    }
    return state_.getEntries();
  }();
  const auto sorted_values = [&state_copy] {
    auto values = state_copy | ranges::views::transform(&decltype(state_)::Entry::value) | ranges::to<std::vector>;
    std::sort(std::begin(values), std::end(values));
    return values;
  }();
  calculateAndSetAttributes(*flow_file, sorted_values);
  session->transfer(flow_file, Success);
}

/**
 * Calculate statistical properties of the values in the rolling window and set them as attributes on the flow file.
 * Properties: count, value (sum), mean (average), median, variance, stddev
 */
void AttributeRollingWindow::calculateAndSetAttributes(core::FlowFile& flow_file, std::span<const double> sorted_values) {
  flow_file.setAttribute("rolling_window_count", std::to_string(sorted_values.size()));
  const auto sum = std::accumulate(std::begin(sorted_values), std::end(sorted_values), 0.0);
  flow_file.setAttribute("rolling_window_value", std::to_string(sum));
  const auto mean = sum / gsl::narrow_cast<double>(sorted_values.size());
  flow_file.setAttribute("rolling_window_mean", std::to_string(mean));
  const auto median = [&] {
    if (sorted_values.size() % 2 == 0) {
      const auto mid = sorted_values.size() / 2;
      return (sorted_values[mid] + sorted_values[mid - 1]) / 2;
    } else {
      return sorted_values[sorted_values.size() / 2];
    }
  }();
  flow_file.setAttribute("rolling_window_median", std::to_string(median));
  const auto variance = std::accumulate(std::begin(sorted_values), std::end(sorted_values), 0.0, [&](double acc, double value) {
    return acc + std::pow(value - mean, 2) / gsl::narrow_cast<double>(sorted_values.size());
  });
  flow_file.setAttribute("rolling_window_variance", std::to_string(variance));
  const auto stddev = std::sqrt(variance);
  flow_file.setAttribute("rolling_window_stddev", std::to_string(stddev));
}

REGISTER_RESOURCE(AttributeRollingWindow, Processor);

} // org::apache::nifi::minifi::processors