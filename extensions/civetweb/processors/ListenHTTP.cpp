/**
 * @file ListenHTTP.cpp

 * ListenHTTP class implementation
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
#include "ListenHTTP.h"

#include <vector>

#include "utils/OptionalUtils.h"
#include "utils/GeneralUtils.h"

namespace org {
namespace apache {
namespace nifi {
namespace minifi {
namespace processors {

core::Property ListenHTTP::BasePath(
    core::PropertyBuilder::createProperty("Base Path")
        ->withDescription("Base path for incoming connections")
        ->isRequired(false)
        ->withDefaultValue<std::string>("contentListener")->build());

core::Property ListenHTTP::Port(
    core::PropertyBuilder::createProperty("Listening Port")
        ->withDescription("The Port to listen on for incoming connections. 0 means port is going to be selected randomly.")
        ->isRequired(true)
        ->withDefaultValue<int>(80, core::StandardValidators::LISTEN_PORT_VALIDATOR())->build());

core::Property ListenHTTP::AuthorizedDNPattern("Authorized DN Pattern", "A Regular Expression to apply against the Distinguished Name of incoming"
                                               " connections. If the Pattern does not match the DN, the connection will be refused.",
                                               ".*");
core::Property ListenHTTP::SSLCertificate("SSL Certificate", "File containing PEM-formatted file including TLS/SSL certificate and key", "");
core::Property ListenHTTP::SSLCertificateAuthority("SSL Certificate Authority", "File containing trusted PEM-formatted certificates", "");

core::Property ListenHTTP::SSLVerifyPeer(
    core::PropertyBuilder::createProperty("SSL Verify Peer")
        ->withDescription("Whether or not to verify the client's certificate (yes/no)")
        ->isRequired(false)
        ->withAllowableValues<std::string>({"yes", "no"})
        ->withDefaultValue("no")->build());

core::Property ListenHTTP::SSLMinimumVersion(
    core::PropertyBuilder::createProperty("SSL Minimum Version")
        -> withDescription("Minimum TLS/SSL version allowed (TLS1.2)")
        ->isRequired(false)
        ->withAllowableValues<std::string>({"TLS1.2"})
        ->withDefaultValue("TLS1.2")->build());

core::Property ListenHTTP::HeadersAsAttributesRegex("HTTP Headers to receive as Attributes (Regex)", "Specifies the Regular Expression that determines the names of HTTP Headers that"
                                                    " should be passed along as FlowFile attributes",
                                                    "");

core::Relationship ListenHTTP::Success("success", "All files are routed to success");

namespace {
// tokenize URL
using TokenType = ListenHTTP::TokenType;

std::string str_from_span(const gsl::span<const char> s) {
  return std::string{s.begin(), s.size()};
}

namespace pattern {
using Token = ListenHTTP::UrlPatternToken;
struct TokenizeResult {
  utils::optional<Token> token;
  const char* end;
};

TokenizeResult get_next_token(const gsl::span<const char> str) {
  if (str.empty()) return {utils::nullopt, str.end()};
  const auto str_until = [str](const char* const end) { return str.subspan(0, std::distance(str.begin(), end)); };
  switch (str[0]) {
    case '/': return {Token{TokenType::Slash, "/"}, str.subspan(1).begin()};
    case '{': {
      const auto closing_brace = std::find(str.begin(), str.end(), '}');
      const auto text_span = str_until(closing_brace + 1);
      if (closing_brace == str.end()) {
        throw std::out_of_range{utils::StringUtils::join_pack("Invalid token: ", str_from_span(text_span))};
      }
      return {Token{TokenType::Argument, str_from_span(text_span)}, closing_brace};
    }
    default: {
      const auto is_slash_or_brace = [](char c) { return c == '/' || c == '{'; };
      const auto text_end = std::find_if(str.begin(), str.end(), is_slash_or_brace);
      const auto text_span = str_until(text_end);
      return {Token{TokenType::String, str_from_span(text_span)}, text_end};
    }
  }
}
}  // namespace pattern

namespace url {
struct UrlComponentDispatcher;

struct UrlComponent {
  virtual void accept(UrlComponentDispatcher&) = 0;
};

struct StringComponent;
struct Argument;
struct Slash;

struct UrlComponentDispatcher {
  virtual void operator()(StringComponent&) = 0;
  virtual void operator()(Argument&) = 0;
  virtual void operator()(Slash&) = 0;
};

struct StringComponent : UrlComponent {
  explicit StringComponent(std::string text) :text{std::move(text)} {}

  void accept(UrlComponentDispatcher& dispatcher) override { dispatcher(*this); }

  std::string text;
};

struct Argument : UrlComponent {
  Argument(std::string key, std::string value) :key{std::move(key)}, value{std::move(value)} {}

  void accept(UrlComponentDispatcher& dispatcher) override { dispatcher(*this); }

  std::string key;
  std::string value;
};

struct Slash : UrlComponent {
  void accept(UrlComponentDispatcher& dispatcher) override { dispatcher(*this); }
};

const char* match_slash(const char* const str) {
  if (str[0] == '/') return str + 1;
  return nullptr;
}

const char* match_string(const char* const str, const pattern::Token& token) {
  const auto mismatch = std::mismatch(token.text.begin(), token.text.end(), str);
  if (mismatch.first != token.text.end()) return nullptr;
  return mismatch.second;
}

std::pair<Argument, const char*> match_argument(const char* const str, const pattern::Token& token, const pattern::Token* next_token) {
  const auto key = token.text.substr(1, token.text.size() - 2);
  if (!next_token) {
    const auto len = std::strlen(str);
    return std::make_pair(Argument{key, std::string{str}}, str + len);
  }

  const auto match_next_token = [next_token](const char* const str) {
    if (next_token->token_type == TokenType::Slash) return match_slash(str);
    else if(next_token->token_type == TokenType::String) return match_string(str, *next_token);
    else throw std::out_of_range{"Cannot have two consecutive argument patterns"};
  };

  // search for the next token, which designates the end of the argument
  const char* it = str;
  while(it && *it && !match_next_token(it)) {
    ++it;
  }

  auto value = str_from_span(gsl::make_span(str, it));
  return std::make_pair(Argument{key, std::move(value)}, it);
}

utils::optional<std::vector<std::unique_ptr<UrlComponent>>> parse_url(const std::string& url, const std::vector<pattern::Token>& pattern) {
  utils::optional<std::vector<std::unique_ptr<UrlComponent>>> result{ std::vector<std::unique_ptr<UrlComponent>>{} };
  const char* iterator = url.c_str();
  auto pattern_it = pattern.begin();
  const char* const last = url.c_str() + url.size();
  while (iterator != last && pattern_it != pattern.end()) {
    switch (pattern_it->token_type) {
      case TokenType::String: {
        auto end = match_string(iterator, *pattern_it);
        if (!end) return utils::nullopt;
        result->push_back(utils::make_unique<StringComponent>(str_from_span(gsl::make_span(iterator, end))));
        iterator = end;
      }
      case TokenType::Slash: {
        auto end = match_slash(iterator);
        if (!end) return utils::nullopt;
        result->push_back(utils::make_unique<Slash>());
        iterator = end;
      }
      case TokenType::Argument: {
        const auto next_token = std::next(pattern_it);
        auto res = match_argument(iterator, *pattern_it, next_token == pattern.end() ? nullptr : &*next_token);
        result->push_back(utils::make_unique<Argument>(std::move(res.first)));
        iterator = res.second;
      }
    }
    ++pattern_it;
  }
  return result;
}

template<typename F>
struct ArgumentCollectorVisitor : url::UrlComponentDispatcher {
  explicit ArgumentCollectorVisitor(F consume) :consume{std::move(consume)} {}

  void operator()(url::StringComponent&) override {}
  void operator()(url::Argument& arg) override {
    consume(arg);
  }
  void operator()(url::Slash&) override {}

  F consume;
};

template<typename F>
ArgumentCollectorVisitor<F> make_arg_collector(F consume) {
  return ArgumentCollectorVisitor<F>{consume};
}
}  // namespace url

std::vector<pattern::Token> tokenize_url_pattern(const std::string& url_pattern) {
  std::vector<pattern::Token> result;
  const char* iterator = url_pattern.c_str();
  const char* const last = url_pattern.c_str() + url_pattern.size();
  while (iterator != last) {
    auto tr = pattern::get_next_token(gsl::make_span(iterator, last));
    if (tr.token) {
      result.push_back(std::move(*tr.token));
      if (result.size() > 1 && result[result.size() - 2].token_type == TokenType::Argument && result.back().token_type == TokenType::Argument) {
        throw std::domain_error{"Cannot have two arguments next to each other in an URL pattern"};
      }
    }
    iterator = tr.end;
  }
  return result;
}

utils::optional<std::vector<std::pair<std::string, std::string>>> match_url(const std::string& url, const std::vector<pattern::Token>& pattern) {
  return url::parse_url(url, pattern) | utils::map([](std::vector<std::unique_ptr<url::UrlComponent>>&& components) {
    std::vector<std::pair<std::string, std::string>> args;
    auto collector = url::make_arg_collector([args](url::Argument& arg) mutable { args.emplace_back(arg.key, arg.value); });
    for (auto& component: components) {
      component->accept(collector);
    }
    return args;
  });
}

std::string static_head(const std::string& url_pattern) {
  const auto brace = url_pattern.find('{');
  if (brace == std::string::npos) return url_pattern;
  const auto last_slash_before_brace = url_pattern.find_last_of('/', brace);
  return url_pattern.substr(0, last_slash_before_brace);
}
}  // namespace

void ListenHTTP::initialize() {
  logger_->log_trace("Initializing ListenHTTP");

  // Set the supported properties
  std::set<core::Property> properties;
  properties.insert(BasePath);
  properties.insert(Port);
  properties.insert(AuthorizedDNPattern);
  properties.insert(SSLCertificate);
  properties.insert(SSLCertificateAuthority);
  properties.insert(SSLVerifyPeer);
  properties.insert(SSLMinimumVersion);
  properties.insert(HeadersAsAttributesRegex);
  setSupportedProperties(properties);
  // Set the supported relationships
  std::set<core::Relationship> relationships;
  relationships.insert(Success);
  setSupportedRelationships(relationships);
}

void ListenHTTP::onSchedule(core::ProcessContext *context, core::ProcessSessionFactory *sessionFactory) {
  std::string basePath;

  if (!context->getProperty(BasePath.getName(), basePath)) {
    logger_->log_info("%s attribute is missing, so default value of %s will be used", BasePath.getName(), BasePath.getValue().to_string());
    basePath = BasePath.getValue().to_string();
  }

  basePath.insert(0, "/");
  url_pattern_ = tokenize_url_pattern(basePath);

  if (!context->getProperty(Port.getName(), listeningPort)) {
    logger_->log_error("%s attribute is missing or invalid", Port.getName());
    return;
  }

  bool randomPort = listeningPort == "0";

  std::string authDNPattern;

  if (context->getProperty(AuthorizedDNPattern.getName(), authDNPattern) && !authDNPattern.empty()) {
    logger_->log_debug("ListenHTTP using %s: %s", AuthorizedDNPattern.getName(), authDNPattern);
  }

  std::string sslCertFile;

  if (context->getProperty(SSLCertificate.getName(), sslCertFile) && !sslCertFile.empty()) {
    logger_->log_debug("ListenHTTP using %s: %s", SSLCertificate.getName(), sslCertFile);
  }

  // Read further TLS/SSL options only if TLS/SSL usage is implied by virtue of certificate value being set
  std::string sslCertAuthorityFile;
  std::string sslVerifyPeer;
  std::string sslMinVer;

  if (!sslCertFile.empty()) {
    if (context->getProperty(SSLCertificateAuthority.getName(), sslCertAuthorityFile) && !sslCertAuthorityFile.empty()) {
      logger_->log_debug("ListenHTTP using %s: %s", SSLCertificateAuthority.getName(), sslCertAuthorityFile);
    }

    if (context->getProperty(SSLVerifyPeer.getName(), sslVerifyPeer)) {
      if (sslVerifyPeer.empty() || sslVerifyPeer.compare("no") == 0) {
        logger_->log_debug("ListenHTTP will not verify peers");
      } else {
        logger_->log_debug("ListenHTTP will verify peers");
      }
    } else {
      logger_->log_debug("ListenHTTP will not verify peers");
    }

    if (context->getProperty(SSLMinimumVersion.getName(), sslMinVer)) {
      logger_->log_debug("ListenHTTP using %s: %s", SSLMinimumVersion.getName(), sslMinVer);
    }
  }

  std::string headersAsAttributesPattern;

  if (context->getProperty(HeadersAsAttributesRegex.getName(), headersAsAttributesPattern) && !headersAsAttributesPattern.empty()) {
    logger_->log_debug("ListenHTTP using %s: %s", HeadersAsAttributesRegex.getName(), headersAsAttributesPattern);
  }

  auto numThreads = getMaxConcurrentTasks();

  const auto listen_path = static_head(basePath);
  logger_->log_info("ListenHTTP starting HTTP server on port %s and path %s with %d threads", randomPort ? "random" : listeningPort, listen_path, numThreads);

  // Initialize web server
  std::vector<std::string> options;
  options.emplace_back("enable_keep_alive");
  options.emplace_back("yes");
  options.emplace_back("keep_alive_timeout_ms");
  options.emplace_back("15000");
  options.emplace_back("num_threads");
  options.emplace_back(std::to_string(numThreads));

  if (sslCertFile.empty()) {
    options.emplace_back("listening_ports");
    options.emplace_back(listeningPort);
  } else {
    listeningPort += "s";
    options.emplace_back("listening_ports");
    options.emplace_back(listeningPort);

    options.emplace_back("ssl_certificate");
    options.emplace_back(sslCertFile);

    if (!sslCertAuthorityFile.empty()) {
      options.emplace_back("ssl_ca_file");
      options.emplace_back(sslCertAuthorityFile);
    }

    if (sslVerifyPeer.empty() || sslVerifyPeer == "no") {
      options.emplace_back("ssl_verify_peer");
      options.emplace_back("no");
    } else {
      options.emplace_back("ssl_verify_peer");
      options.emplace_back("yes");
    }

    if (sslMinVer == "TLS1.2") {
      options.emplace_back("ssl_protocol_version");
      options.emplace_back(std::to_string(4));
    } else {
      throw minifi::Exception(ExceptionType::PROCESSOR_EXCEPTION, "Invalid SSL Minimum Version specified!");
    }
  }

  server_.reset(new CivetServer(options, &callbacks_, &logger_));
  handler_.reset(new Handler(listen_path, context, sessionFactory, std::move(authDNPattern), std::move(headersAsAttributesPattern), url_pattern_));
  server_->addHandler(listen_path, handler_.get());

  if (randomPort) {
    const auto& vec = server_->getListeningPorts();
    if (vec.size() != 1) {
      logger_->log_error("Random port is set, but there is no listening port! Server most probably failed to start!");
    } else {
      bool is_secure = isSecure();
      listeningPort = std::to_string(vec[0]);
      if (is_secure) {
        listeningPort += "s";
      }
      logger_->log_info("Listening on port %s", listeningPort);
    }
  }
}

ListenHTTP::~ListenHTTP() = default;

void ListenHTTP::onTrigger(core::ProcessContext *context, core::ProcessSession *session) {
  std::shared_ptr<FlowFileRecord> flow_file = std::static_pointer_cast<FlowFileRecord>(session->get());

  // Do nothing if there are no incoming files
  if (!flow_file) {
    return;
  }

  std::string type;
  flow_file->getAttribute("http.type", type);

  if (type == "response_body") {

    if (handler_) {
      struct response_body response { "", "", "" };
      ResponseBodyReadCallback cb(&response.body);
      flow_file->getAttribute("filename", response.uri);
      flow_file->getAttribute("mime.type", response.mime_type);
      if (response.mime_type.empty()) {
        logger_->log_warn("Using default mime type of application/octet-stream for response body file: %s", response.uri);
        response.mime_type = "application/octet-stream";
      }
      session->read(flow_file, &cb);
      handler_->set_response_body(std::move(response));
    }
  }

  session->remove(flow_file);
}

ListenHTTP::Handler::Handler(std::string base_uri,
    core::ProcessContext *context,
    core::ProcessSessionFactory *session_factory,
    const std::string &auth_dn_regex,
    const std::string &header_as_attrs_regex,
    std::vector<UrlPatternToken>& pattern)
    : base_uri_(std::move(base_uri)),
      auth_dn_regex_(auth_dn_regex),
      headers_as_attrs_regex_(header_as_attrs_regex),
      pattern_(gsl::make_not_null(&pattern)),
      logger_(logging::LoggerFactory<ListenHTTP::Handler>::getLogger()) {
  process_context_ = context;
  session_factory_ = session_factory;
}

void ListenHTTP::Handler::send_error_response(struct mg_connection *conn) {
  mg_printf(conn, "HTTP/1.1 500 Internal Server Error\r\n"
            "Content-Type: text/html\r\n"
            "Content-Length: 0\r\n\r\n");
}

void ListenHTTP::Handler::set_header_attributes(const mg_request_info *req_info, const std::shared_ptr<FlowFileRecord> &flow_file) const {
  // Add filename from "filename" header value (and pattern headers)
  for (int i = 0; i < req_info->num_headers; i++) {
    auto header = &req_info->http_headers[i];

    if (strcmp("filename", header->name) == 0) {
      if (!flow_file->updateAttribute("filename", header->value)) {
        flow_file->addAttribute("filename", header->value);
      }
    } else if (std::regex_match(header->name, headers_as_attrs_regex_)) {
      if (!flow_file->updateAttribute(header->name, header->value)) {
        flow_file->addAttribute(header->name, header->value);
      }
    }
  }

  if (req_info->query_string) {
    flow_file->addAttribute("http.query", req_info->query_string);
  }
}

bool ListenHTTP::Handler::handlePost(CivetServer *server, struct mg_connection *conn) {
  auto req_info = mg_get_request_info(conn);
  if (!req_info) {
      logger_->log_error("ListenHTTP handling POST resulted in a null request");
      return false;
  }
  logger_->log_debug("ListenHTTP handling POST request of length %lld", req_info->content_length);

  if (!auth_request(conn, req_info)) {
    return true;
  }

  const auto url_arguments = match_url(req_info->request_uri, *pattern_);

  // Always send 100 Continue, as allowed per standard to minimize client delay (https://www.w3.org/Protocols/rfc2616/rfc2616-sec8.html)
  mg_printf(conn, "HTTP/1.1 100 Continue\r\n\r\n");

  auto session = session_factory_->createSession();
  ListenHTTP::WriteCallback callback(conn, req_info);
  auto flow_file = std::static_pointer_cast<FlowFileRecord>(session->create());

  if (!flow_file) {
    send_error_response(conn);
    return true;
  }

  try {
    session->write(flow_file, &callback);
    set_header_attributes(req_info, flow_file);
    if (url_arguments) {
      for(const auto& arg: *url_arguments) {
        flow_file->addAttribute(arg.first, arg.second);
      }
    }
    session->transfer(flow_file, Success);
    session->commit();
  } catch (std::exception &exception) {
    logger_->log_error("ListenHTTP Caught Exception %s", exception.what());
    send_error_response(conn);
    session->rollback();
    throw;
  } catch (...) {
    logger_->log_error("ListenHTTP Caught Exception Processor::onTrigger");
    send_error_response(conn);
    session->rollback();
    throw;
  }

  mg_printf(conn, "HTTP/1.1 200 OK\r\n");
  write_body(conn, req_info);

  return true;
}

bool ListenHTTP::Handler::auth_request(mg_connection *conn, const mg_request_info *req_info) const {
  // If this is a two-way TLS connection, authorize the peer against the configured pattern
  bool authorized = true;
  if (req_info->is_ssl && req_info->client_cert != nullptr) {
    if (!std::regex_match(req_info->client_cert->subject, auth_dn_regex_)) {
      mg_printf(conn, "HTTP/1.1 403 Forbidden\r\n"
                "Content-Type: text/html\r\n"
                "Content-Length: 0\r\n\r\n");
      logger_->log_warn("ListenHTTP client DN not authorized: %s", req_info->client_cert->subject);
      authorized = false;
    }
  }
  return authorized;
}

bool ListenHTTP::Handler::handleGet(CivetServer *server, struct mg_connection *conn) {
  auto req_info = mg_get_request_info(conn);
  if (!req_info) {
      logger_->log_error("ListenHTTP handling GET resulted in a null request");
      return false;
  }
  logger_->log_debug("ListenHTTP handling GET request of URI %s", req_info->request_uri);

  if (!auth_request(conn, req_info)) {
    return true;
  }

  auto session = session_factory_->createSession();
  auto flow_file = std::static_pointer_cast<FlowFileRecord>(session->create());

  if (!flow_file) {
    send_error_response(conn);
    return true;
  }

  try {
    set_header_attributes(req_info, flow_file);

    session->transfer(flow_file, Success);
    session->commit();
  } catch (std::exception &exception) {
    logger_->log_error("ListenHTTP Caught Exception %s", exception.what());
    send_error_response(conn);
    session->rollback();
    throw;
  } catch (...) {
    logger_->log_error("ListenHTTP Caught Exception Processor::onTrigger");
    send_error_response(conn);
    session->rollback();
    throw;
  }

  mg_printf(conn, "HTTP/1.1 200 OK\r\n");
  write_body(conn, req_info);

  return true;
}

bool ListenHTTP::Handler::handleHead(CivetServer *server, struct mg_connection *conn) {
  auto req_info = mg_get_request_info(conn);
  if (!req_info) {
    logger_->log_error("ListenHTTP handling HEAD resulted in a null request");
    return false;
  }
  logger_->log_debug("ListenHTTP handling HEAD request of URI %s", req_info->request_uri);

  if (!auth_request(conn, req_info)) {
    return true;
  }

  mg_printf(conn, "HTTP/1.1 200 OK\r\n");
  write_body(conn, req_info, false /*include_payload*/);

  return true;
}

void ListenHTTP::Handler::write_body(mg_connection *conn, const mg_request_info *req_info, bool include_payload /*=true*/) {
  const auto &request_uri_str = std::string(req_info->request_uri);

  if (request_uri_str.size() > base_uri_.size() + 1) {
    struct response_body response { };

    {
      // Attempt to minimize time holding mutex (it would be nice to have a lock-free concurrent map here)
      std::lock_guard<std::mutex> guard(uri_map_mutex_);
      std::string req_uri = request_uri_str.substr(base_uri_.size() + 1);

      if (response_uri_map_.count(req_uri)) {
        response = response_uri_map_[req_uri];
      }
    }

    if (!response.body.empty()) {
      logger_->log_debug("Writing response body of %lu bytes for URI: %s", response.body.size(), req_info->request_uri);
      mg_printf(conn, "Content-type: ");
      mg_printf(conn, "%s", response.mime_type.c_str());
      mg_printf(conn, "\r\n");
      mg_printf(conn, "Content-length: ");
      mg_printf(conn, "%s", std::to_string(response.body.size()).c_str());
      mg_printf(conn, "\r\n\r\n");
      if (include_payload) {
        mg_printf(conn, "%s", response.body.c_str());
      }
    } else {
      logger_->log_debug("No response body available for URI: %s", req_info->request_uri);
      mg_printf(conn, "Content-length: 0\r\n\r\n");
    }
  } else {
    logger_->log_debug("No response body available for URI: %s", req_info->request_uri);
    mg_printf(conn, "Content-length: 0\r\n\r\n");
  }
}

ListenHTTP::WriteCallback::WriteCallback(struct mg_connection *conn, const struct mg_request_info *reqInfo)
    : logger_(logging::LoggerFactory<ListenHTTP::WriteCallback>::getLogger()) {
  conn_ = conn;
  req_info_ = reqInfo;
}

int64_t ListenHTTP::WriteCallback::process(std::shared_ptr<io::BaseStream> stream) {
  int64_t rlen;
  int64_t nlen = 0;
  int64_t tlen = req_info_->content_length;
  uint8_t buf[16384];

  // if we have no content length we should call mg_read until
  // there is no data left from the stream to be HTTP/1.1 compliant
  while (tlen == -1 || nlen < tlen) {
    rlen = tlen == -1 ? sizeof(buf) : tlen - nlen;

    if (rlen > (int64_t) sizeof(buf)) {
      rlen = (int64_t) sizeof(buf);
    }

    // Read a buffer of data from client
    rlen = mg_read(conn_, &buf[0], (size_t) rlen);

    if (rlen <= 0) {
      break;
    }

    // Transfer buffer data to the output stream
    stream->write(&buf[0], gsl::narrow<int>(rlen));

    nlen += rlen;
  }

  return nlen;
}

bool ListenHTTP::isSecure() const {
  return (listeningPort.length() > 0) && *listeningPort.rbegin() == 's';
}

std::string ListenHTTP::getPort() const {
  if(isSecure()) {
    return listeningPort.substr(0, listeningPort.length() -1);
  }
  return listeningPort;
}

void ListenHTTP::notifyStop() {
  server_.reset();
  handler_.reset();
}

} /* namespace processors */
} /* namespace minifi */
} /* namespace nifi */
} /* namespace apache */
} /* namespace org */
