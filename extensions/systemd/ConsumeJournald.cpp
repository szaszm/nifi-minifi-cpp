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

#include "ConsumeJournald.h"

#include <algorithm>

#include "fmt/format.h"
#include <systemd/sd-journal.h>
#include "utils/GeneralUtils.h"

namespace org { namespace apache { namespace nifi { namespace minifi { namespace extensions { namespace systemd {

static constexpr const char* DATETIME_FORMAT = "%x %X %Z";

constexpr const char* ConsumeJournald::CURSOR_KEY;
const core::Relationship ConsumeJournald::Success("success", "Successfully consumed journal messages.");

const core::Property ConsumeJournald::BatchSize = core::PropertyBuilder::createProperty("Batch Size")
    ->withDescription("The maximum number of entries processed in a single execution.")
    ->withDefaultValue<size_t>(1000)
    ->isRequired(true)
    ->build();

const core::Property ConsumeJournald::PayloadFormat = core::PropertyBuilder::createProperty("Payload Format")
    ->withDescription("Configures flow file content formatting. Raw: only the message. Syslog: similar to syslog or journalctl output.")
    ->withDefaultValue<std::string>(PAYLOAD_FORMAT_SYSLOG)
    ->withAllowableValues<std::string>({PAYLOAD_FORMAT_RAW, PAYLOAD_FORMAT_SYSLOG})
    ->isRequired(true)
    ->build();

const core::Property ConsumeJournald::IncludeTimestamp = core::PropertyBuilder::createProperty("Include Timestamp")
    ->withDescription("Include message timestamp in the 'timestamp' attribute")
    ->withDefaultValue<bool>(true)
    ->isRequired(true)
    ->build();

const core::Property ConsumeJournald::JournalType = core::PropertyBuilder::createProperty("Journal Type")
    ->withDescription("Type of journal to consume")
    ->withDefaultValue<std::string>(JOURNAL_TYPE_SYSTEM)
    ->withAllowableValues<std::string>({JOURNAL_TYPE_USER, JOURNAL_TYPE_SYSTEM, JOURNAL_TYPE_BOTH})
    ->isRequired(true)
    ->build();

ConsumeJournald::ConsumeJournald(const std::string &name, const utils::Identifier &id, std::unique_ptr<libwrapper::LibWrapper>&& libwrapper)
    :core::Processor{name, id}, libwrapper_{std::move(libwrapper)}
{}

void ConsumeJournald::initialize() {
  setSupportedProperties({BatchSize});
  // Set the supported relationships
  setSupportedRelationships({Success});

  worker_ = utils::make_unique<Worker>();
}

void ConsumeJournald::notifyStop() {
  bool running = true;
  if (!running_.compare_exchange_strong(running, false, std::memory_order_acq_rel) || !journal_handle_) return;
  worker_->enqueue([this] {
    journal_handle_.reset();
  }).get();
  worker_ = nullptr;
}

void ConsumeJournald::onSchedule(core::ProcessContext* const context, core::ProcessSessionFactory* const sessionFactory) {
  gsl_Expects(context && sessionFactory && !running_ && worker_);
  using JournalTypeEnum = systemd::JournalType;

  const auto parse_payload_format = [](const std::string& property_value) -> utils::optional<systemd::PayloadFormat> {
    if (utils::StringUtils::equalsIgnoreCase(property_value, PAYLOAD_FORMAT_RAW)) return systemd::PayloadFormat::Raw;
    if (utils::StringUtils::equalsIgnoreCase(property_value, PAYLOAD_FORMAT_SYSLOG)) return systemd::PayloadFormat::Syslog;
    return utils::nullopt;
  };
  const auto parse_journal_type = [](const std::string& property_value) -> utils::optional<JournalTypeEnum> {
    if (utils::StringUtils::equalsIgnoreCase(property_value, JOURNAL_TYPE_USER)) return JournalTypeEnum::User;
    if (utils::StringUtils::equalsIgnoreCase(property_value, JOURNAL_TYPE_SYSTEM)) return JournalTypeEnum::System;
    if (utils::StringUtils::equalsIgnoreCase(property_value, JOURNAL_TYPE_BOTH)) return JournalTypeEnum::Both;
    return utils::nullopt;
  };
  batch_size_ = context->getProperty<size_t>(BatchSize).value_or(10);
  payload_format_ = (context->getProperty(PayloadFormat) | utils::flatMap(parse_payload_format)).value_or(systemd::PayloadFormat::Syslog);
  include_timestamp_ = context->getProperty<bool>(IncludeTimestamp).value_or(true);
  const auto journal_type = (context->getProperty(JournalType) | utils::flatMap(parse_journal_type)).value_or(JournalTypeEnum::System);

  state_manager_ = context->getStateManager();
  journal_handle_ = utils::make_optional(worker_->enqueue([this, journal_type]{
    return JournalHandle{libwrapper_->openJournal(journal_type)};
  }).get());
  worker_->enqueue([this] {
    journal_handle_->visit([this](libwrapper::Journal& journal) {
      const auto cursor = state_manager_->get() | utils::map([](std::unordered_map<std::string, std::string>&& m) { return m.at(CURSOR_KEY); });
      if (!cursor) {
        journal.seekTail();
      } else {
        const auto ret = journal.seekCursor(cursor->c_str());
        if (ret < 0) {
          const auto error_message = std::generic_category().default_error_condition(-ret).message();
          logger_->log_warn("Failed to seek to cursor: %s. Seeking to tail instead. cursor=\"%s\"", error_message, *cursor);
          journal.seekTail();
        }
      }
    });
  }).get();
  running_ = true;
}

void ConsumeJournald::onTrigger(core::ProcessContext* const context, core::ProcessSession* const session) {
  gsl_Expects(context && session);
  if (!running_.load(std::memory_order_acquire)) return;
  auto cursor_and_messages = getCursorAndMessageBatch().get();
  state_manager_->set({{"cursor", std::move(cursor_and_messages.first)}});
  auto messages = std::move(cursor_and_messages.second);

  if (messages.empty()) {
    yield();
    return;
  }

  for (auto& msg: messages) {
    const auto flow_file = session->create();
    if (payload_format_ == systemd::PayloadFormat::Syslog) session->writeBuffer(flow_file, gsl::make_span(formatSyslogMessage(msg)));
    for (auto& field: msg.fields) {
      if (field.name == "MESSAGE" && payload_format_ == systemd::PayloadFormat::Raw) {
        session->writeBuffer(flow_file, gsl::make_span(field.value));
      } else {
        flow_file->setAttribute(std::move(field.name), std::move(field.value));
      }
    }
    if (include_timestamp_) flow_file->setAttribute("timestamp", date::format(DATETIME_FORMAT, msg.timestamp));
    session->transfer(flow_file, Success);
  }
  session->commit();
}

utils::optional<gsl::span<const char>> ConsumeJournald::enumerateJournalEntry(libwrapper::Journal& journal) {
  const void* data_ptr{};
  size_t data_length{};
  const auto status_code = journal.enumerateData(&data_ptr, &data_length);
  if (status_code == 0) return {};
  if (status_code < 0) throw SystemErrorException{ "sd_journal_enumerate_data", std::generic_category().default_error_condition(-status_code) };
  gsl_Ensures(data_ptr && "if sd_journal_enumerate_data was successful, then data_ptr must be set");
  gsl_Ensures(data_length > 0 && "if sd_journal_enumerate_data was successful, then data_length must be greater than zero");
  const char* const data_str_ptr = reinterpret_cast<const char*>(data_ptr);
  return gsl::make_span(data_str_ptr, data_length);
}

utils::optional<ConsumeJournald::journal_field> ConsumeJournald::getNextField(libwrapper::Journal& journal) {
  return enumerateJournalEntry(journal) | utils::map([](gsl::span<const char> field) {
    const auto eq_pos = std::find(std::begin(field), std::end(field), '=');
    gsl_Ensures(eq_pos != std::end(field) && "field string must contain an equals sign");
    const auto eq_idx = gsl::narrow<size_t>(eq_pos - std::begin(field));
    return journal_field{
        utils::span_to<std::string>(field.subspan(0, eq_idx)),
        utils::span_to<std::string>(field.subspan(eq_idx + 1))
    };
  });
}

std::future<std::pair<std::string, std::vector<ConsumeJournald::journal_message>>> ConsumeJournald::getCursorAndMessageBatch() {
  gsl_Expects(worker_);
  return worker_->enqueue([this] {
    std::vector<journal_message> messages;
    messages.reserve(batch_size_);
    std::unique_ptr<char, utils::FreeDeleter> cursor;
    journal_handle_->visit([this, &messages, &cursor](libwrapper::Journal& journal) {
      for (size_t i = 0; i < batch_size_ && journal.next() > 0; ++i) {
        journal_message message;
        utils::optional<journal_field> field;
        while ((field = getNextField(journal)).has_value()) {
          message.fields.push_back(std::move(*field));
        }
        if (include_timestamp_ || payload_format_ == systemd::PayloadFormat::Syslog) {
          message.timestamp = [&journal] {
            uint64_t journal_timestamp_usec_since_epoch{};
            journal.getRealtimeUsec(&journal_timestamp_usec_since_epoch);
            return std::chrono::system_clock::time_point{std::chrono::microseconds{journal_timestamp_usec_since_epoch}};
          }();
        }
        messages.push_back(std::move(message));
      }

      char* cursor_out;
      const auto err_code = journal.getCursor(&cursor_out);
      if (err_code < 0) throw SystemErrorException{"sd_journal_get_cursor", std::generic_category().default_error_condition(-err_code)};
      gsl_Ensures(cursor_out);
      cursor.reset(cursor_out);
    });
    return std::make_pair(std::string{cursor.get()}, messages);
  });
}

std::string ConsumeJournald::formatSyslogMessage(const journal_message& msg) {
  gsl_Expects(msg.timestamp != decltype(msg.timestamp){});

  const std::string* systemd_hostname = nullptr;
  const std::string* syslog_pid = nullptr;
  const std::string* systemd_pid = nullptr;
  const std::string* syslog_identifier = nullptr;
  const std::string* message = nullptr;

  for (const auto& field: msg.fields) {
    if (field.name == "_HOSTNAME") systemd_hostname = &field.value;
    else if (field.name == "SYSLOG_PID") syslog_pid = &field.value;
    else if (field.name == "_PID") systemd_pid = &field.value;
    else if (field.name == "SYSLOG_IDENTIFIER") syslog_identifier = &field.value;
    else if (field.name == "MESSAGE") message = &field.value;
    else if (systemd_hostname && (syslog_pid || systemd_pid) && syslog_identifier && message) break;
  }

  gsl_Ensures(message && "MESSAGE is guaranteed to be present");

  const auto pid_string = utils::optional_from_ptr(syslog_pid)
      | utils::orElse([&] { return utils::optional_from_ptr(systemd_pid); })
      | utils::map([](const std::string* pid) { return fmt::format("[{}]", *pid); });

  return fmt::format("{} {} {}{}: {}",
      date::format(DATETIME_FORMAT, msg.timestamp),
      (utils::optional_from_ptr(systemd_hostname) | utils::map(utils::dereference)).value_or("unknown_host"),
      (utils::optional_from_ptr(syslog_identifier) | utils::map(utils::dereference)).value_or("unknown_process"),
      pid_string.value_or(std::string{}),
      *message);
}

}}}}}}  // namespace org::apache::nifi::minifi::extensions::systemd
