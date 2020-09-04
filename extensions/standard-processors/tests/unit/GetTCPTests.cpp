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
#include <utility>
#include <memory>
#include <string>
#include <vector>
#include <set>
#include <fstream>
#include "unit/ProvenanceTestHelper.h"
#include "TestBase.h"
#include "RandomServerSocket.h"
#include "Scheduling.h"
#include "LogAttribute.h"
#include "GetTCP.h"
#include "core/Core.h"
#include "core/FlowFile.h"
#include "core/Processor.h"
#include "core/ProcessContext.h"
#include "core/ProcessSession.h"
#include "core/ProcessorNode.h"
#include "core/reporting/SiteToSiteProvenanceReportingTask.h"

TEST_CASE("GetTCPWithoutEOM", "[GetTCP1]") {
  utils::Identifier ident = utils::Identifier();

  TestController testController;
  std::vector<uint8_t> buffer;
  for (auto c : "Hello World\nHello Warld\nGoodByte Cruel world") {
    buffer.push_back(c);
  }
  org::apache::nifi::minifi::utils::debug_shared_ptr<core::ContentRepository> content_repo = utils::debug_make_shared<core::repository::VolatileContentRepository>();

  content_repo->initialize(utils::debug_make_shared<minifi::Configure>());

  org::apache::nifi::minifi::utils::debug_shared_ptr<org::apache::nifi::minifi::io::StreamFactory> stream_factory = minifi::io::StreamFactory::getInstance(utils::debug_make_shared<minifi::Configure>());
  org::apache::nifi::minifi::io::RandomServerSocket server(org::apache::nifi::minifi::io::Socket::getMyHostName());

  LogTestController::getInstance().setDebug<minifi::processors::LogAttribute>();
  LogTestController::getInstance().setDebug<minifi::processors::GetTCP>();
  LogTestController::getInstance().setTrace<minifi::io::Socket>();

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::Repository> repo = utils::debug_make_shared<TestRepository>();

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::Processor> processor = utils::debug_make_shared<org::apache::nifi::minifi::processors::GetTCP>("gettcpexample");

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::Processor> logAttribute = utils::debug_make_shared<org::apache::nifi::minifi::processors::LogAttribute>("logattribute");

  processor->setStreamFactory(stream_factory);
  processor->initialize();

  utils::Identifier processoruuid;
  REQUIRE(true == processor->getUUID(processoruuid));

  utils::Identifier logattribute_uuid;
  REQUIRE(true == logAttribute->getUUID(logattribute_uuid));

  REQUIRE(processoruuid.to_string() != logattribute_uuid.to_string());

  org::apache::nifi::minifi::utils::debug_shared_ptr<minifi::Connection> connection = utils::debug_make_shared<minifi::Connection>(repo, content_repo, "gettcpexampleConnection");
  connection->addRelationship(core::Relationship("success", "description"));

  org::apache::nifi::minifi::utils::debug_shared_ptr<minifi::Connection> connection2 = utils::debug_make_shared<minifi::Connection>(repo, content_repo, "logattribute");
  connection2->addRelationship(core::Relationship("success", "description"));

  // link the connections so that we can test results at the end for this
  connection->setSource(processor);

  // link the connections so that we can test results at the end for this
  connection->setDestination(logAttribute);

  connection2->setSource(logAttribute);

  connection2->setSourceUUID(logattribute_uuid);
  connection->setSourceUUID(processoruuid);
  connection->setDestinationUUID(logattribute_uuid);

  processor->addConnection(connection);
  logAttribute->addConnection(connection);
  logAttribute->addConnection(connection2);

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessorNode> node = utils::debug_make_shared<core::ProcessorNode>(processor);
    org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessorNode> node2 = utils::debug_make_shared<core::ProcessorNode>(logAttribute);
    org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessContext> context = utils::debug_make_shared<core::ProcessContext>(node, nullptr, repo, repo, content_repo);
    org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessContext> context2 = utils::debug_make_shared<core::ProcessContext>(node2, nullptr, repo, repo, content_repo);
  context->setProperty(org::apache::nifi::minifi::processors::GetTCP::EndpointList, org::apache::nifi::minifi::io::Socket::getMyHostName() + ":" + std::to_string(server.getPort()));
  context->setProperty(org::apache::nifi::minifi::processors::GetTCP::ReconnectInterval, "100 msec");
  auto session = utils::debug_make_shared<core::ProcessSession>(context);
    auto session2 = utils::debug_make_shared<core::ProcessSession>(context2);

  REQUIRE(processor->getName() == "gettcpexample");

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::FlowFile> record;
  processor->setScheduledState(core::ScheduledState::RUNNING);

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessSessionFactory> factory = utils::debug_make_shared<core::ProcessSessionFactory>(context);
  processor->onSchedule(context, factory);
  processor->onTrigger(context, session);
  server.writeData(buffer, buffer.size());
  std::this_thread::sleep_for(std::chrono::seconds(2));

  logAttribute->initialize();
  logAttribute->incrementActiveTasks();
  logAttribute->setScheduledState(core::ScheduledState::RUNNING);
  org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessSessionFactory> factory2 = utils::debug_make_shared<core::ProcessSessionFactory>(context2);
  logAttribute->onSchedule(context2, factory2);
  logAttribute->onTrigger(context2, session2);

  auto reporter = session->getProvenanceReporter();
  auto records = reporter->getEvents();
  record = session->get();
  REQUIRE(record == nullptr);
  REQUIRE(records.size() == 0);

  processor->incrementActiveTasks();
  processor->setScheduledState(core::ScheduledState::RUNNING);
  processor->onTrigger(context, session);
  reporter = session->getProvenanceReporter();

  records = reporter->getEvents();
  session->commit();

  logAttribute->incrementActiveTasks();
  logAttribute->setScheduledState(core::ScheduledState::RUNNING);
  logAttribute->onTrigger(context2, session2);

  records = reporter->getEvents();

  REQUIRE(true == LogTestController::getInstance().contains("Size:45 Offset:0"));

  LogTestController::getInstance().reset();
}

TEST_CASE("GetTCPWithOEM", "[GetTCP2]") {
  std::vector<uint8_t> buffer;
  for (auto c : "Hello World\nHello Warld\nGoodByte Cruel world") {
    buffer.push_back(c);
  }
  org::apache::nifi::minifi::utils::debug_shared_ptr<core::ContentRepository> content_repo = utils::debug_make_shared<core::repository::VolatileContentRepository>();

  content_repo->initialize(utils::debug_make_shared<minifi::Configure>());

  org::apache::nifi::minifi::utils::debug_shared_ptr<org::apache::nifi::minifi::io::StreamFactory> stream_factory = minifi::io::StreamFactory::getInstance(utils::debug_make_shared<minifi::Configure>());

  TestController testController;

  org::apache::nifi::minifi::io::RandomServerSocket server(org::apache::nifi::minifi::io::Socket::getMyHostName());

  LogTestController::getInstance().setDebug<minifi::processors::LogAttribute>();
  LogTestController::getInstance().setTrace<core::repository::VolatileContentRepository >();
  LogTestController::getInstance().setTrace<minifi::processors::GetTCP>();
  LogTestController::getInstance().setTrace<core::ConfigurableComponent>();
  LogTestController::getInstance().setTrace<minifi::io::Socket>();

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::Repository> repo = utils::debug_make_shared<TestRepository>();

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::Processor> processor = utils::debug_make_shared<org::apache::nifi::minifi::processors::GetTCP>("gettcpexample");

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::Processor> logAttribute = utils::debug_make_shared<org::apache::nifi::minifi::processors::LogAttribute>("logattribute");

  processor->setStreamFactory(stream_factory);
  processor->initialize();

  utils::Identifier processoruuid;
  REQUIRE(true == processor->getUUID(processoruuid));

  utils::Identifier logattribute_uuid;
  REQUIRE(true == logAttribute->getUUID(logattribute_uuid));

  org::apache::nifi::minifi::utils::debug_shared_ptr<minifi::Connection> connection = utils::debug_make_shared<minifi::Connection>(repo, content_repo, "gettcpexampleConnection");
  connection->addRelationship(core::Relationship("partial", "description"));

  org::apache::nifi::minifi::utils::debug_shared_ptr<minifi::Connection> connection2 = utils::debug_make_shared<minifi::Connection>(repo, content_repo, "logattribute");
  connection2->addRelationship(core::Relationship("partial", "description"));

  // link the connections so that we can test results at the end for this
  connection->setSource(processor);

  // link the connections so that we can test results at the end for this
  connection->setDestination(logAttribute);

  connection2->setSource(logAttribute);

  connection2->setSourceUUID(logattribute_uuid);
  connection->setSourceUUID(processoruuid);
  connection->setDestinationUUID(logattribute_uuid);

  processor->addConnection(connection);
  logAttribute->addConnection(connection);
  logAttribute->addConnection(connection2);

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessorNode> node = utils::debug_make_shared<core::ProcessorNode>(processor);
    org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessorNode> node2 = utils::debug_make_shared<core::ProcessorNode>(logAttribute);
    org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessContext> context = utils::debug_make_shared<core::ProcessContext>(node, nullptr, repo, repo, content_repo);
    org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessContext> context2 = utils::debug_make_shared<core::ProcessContext>(node2, nullptr, repo, repo, content_repo);
  context->setProperty(org::apache::nifi::minifi::processors::GetTCP::EndpointList, org::apache::nifi::minifi::io::Socket::getMyHostName() + ":" + std::to_string(server.getPort()));
  context->setProperty(org::apache::nifi::minifi::processors::GetTCP::ReconnectInterval, "100 msec");
  // we're using new lines above
  context->setProperty(org::apache::nifi::minifi::processors::GetTCP::EndOfMessageByte, "10");
  auto session = utils::debug_make_shared<core::ProcessSession>(context);
  auto session2 = utils::debug_make_shared<core::ProcessSession>(context2);


  REQUIRE(processor->getName() == "gettcpexample");

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::FlowFile> record;
  processor->setScheduledState(core::ScheduledState::RUNNING);

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessSessionFactory> factory = utils::debug_make_shared<core::ProcessSessionFactory>(context);
  processor->onSchedule(context, factory);
  processor->onTrigger(context, session);
  server.writeData(buffer, buffer.size());
  std::this_thread::sleep_for(std::chrono::seconds(2));

  logAttribute->initialize();
  logAttribute->incrementActiveTasks();
  logAttribute->setScheduledState(core::ScheduledState::RUNNING);
  org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessSessionFactory> factory2 = utils::debug_make_shared<core::ProcessSessionFactory>(context2);
  logAttribute->onSchedule(context2, factory2);
  logAttribute->onTrigger(context2, session2);

  auto reporter = session->getProvenanceReporter();
  auto records = reporter->getEvents();
  record = session->get();
  REQUIRE(record == nullptr);
  REQUIRE(records.size() == 0);

  processor->incrementActiveTasks();
  processor->setScheduledState(core::ScheduledState::RUNNING);
  processor->onTrigger(context, session);
  reporter = session->getProvenanceReporter();

  records = reporter->getEvents();
  session->commit();

  logAttribute->incrementActiveTasks();
  logAttribute->setScheduledState(core::ScheduledState::RUNNING);
  logAttribute->onTrigger(context2, session2);

  records = reporter->getEvents();

  logAttribute->incrementActiveTasks();
  logAttribute->setScheduledState(core::ScheduledState::RUNNING);
  logAttribute->onTrigger(context2, session2);

  records = reporter->getEvents();

  REQUIRE(true == LogTestController::getInstance().contains("Size:11 Offset:0"));
  REQUIRE(true == LogTestController::getInstance().contains("Size:12 Offset:0"));
  REQUIRE(true == LogTestController::getInstance().contains("Size:22 Offset:0"));

  LogTestController::getInstance().reset();
}

TEST_CASE("GetTCPWithOnlyOEM", "[GetTCP3]") {
  std::vector<uint8_t> buffer;
  for (auto c : "\n") {
    buffer.push_back(c);
  }

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::ContentRepository> content_repo = utils::debug_make_shared<core::repository::VolatileContentRepository>();

  content_repo->initialize(utils::debug_make_shared<minifi::Configure>());

  org::apache::nifi::minifi::utils::debug_shared_ptr<org::apache::nifi::minifi::io::StreamFactory> stream_factory = minifi::io::StreamFactory::getInstance(utils::debug_make_shared<minifi::Configure>());

  TestController testController;

  LogTestController::getInstance().setDebug<minifi::io::Socket>();

  org::apache::nifi::minifi::io::RandomServerSocket server(org::apache::nifi::minifi::io::Socket::getMyHostName());

  LogTestController::getInstance().setDebug<minifi::processors::LogAttribute>();

  LogTestController::getInstance().setDebug<minifi::processors::GetTCP>();

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::Repository> repo = utils::debug_make_shared<TestRepository>();

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::Processor> processor = utils::debug_make_shared<org::apache::nifi::minifi::processors::GetTCP>("gettcpexample");

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::Processor> logAttribute = utils::debug_make_shared<org::apache::nifi::minifi::processors::LogAttribute>("logattribute");

  processor->setStreamFactory(stream_factory);
  processor->initialize();

  utils::Identifier processoruuid;
  REQUIRE(true == processor->getUUID(processoruuid));

  utils::Identifier logattribute_uuid;
  REQUIRE(true == logAttribute->getUUID(logattribute_uuid));

  org::apache::nifi::minifi::utils::debug_shared_ptr<minifi::Connection> connection = utils::debug_make_shared<minifi::Connection>(repo, content_repo, "gettcpexampleConnection");
  connection->addRelationship(core::Relationship("success", "description"));

  org::apache::nifi::minifi::utils::debug_shared_ptr<minifi::Connection> connection2 = utils::debug_make_shared<minifi::Connection>(repo, content_repo, "logattribute");
  connection2->addRelationship(core::Relationship("success", "description"));

  // link the connections so that we can test results at the end for this
  connection->setSource(processor);

  // link the connections so that we can test results at the end for this
  connection->setDestination(logAttribute);

  connection2->setSource(logAttribute);

  connection2->setSourceUUID(logattribute_uuid);
  connection->setSourceUUID(processoruuid);
  connection->setDestinationUUID(logattribute_uuid);

  processor->addConnection(connection);
  logAttribute->addConnection(connection);
  logAttribute->addConnection(connection2);

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessorNode> node = utils::debug_make_shared<core::ProcessorNode>(processor);
    org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessorNode> node2 = utils::debug_make_shared<core::ProcessorNode>(logAttribute);
    org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessContext> context = utils::debug_make_shared<core::ProcessContext>(node, nullptr, repo, repo, content_repo);
    org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessContext> context2 = utils::debug_make_shared<core::ProcessContext>(node2, nullptr, repo, repo, content_repo);
  context->setProperty(org::apache::nifi::minifi::processors::GetTCP::EndpointList, org::apache::nifi::minifi::io::Socket::getMyHostName() + ":" + std::to_string(server.getPort()));
  context->setProperty(org::apache::nifi::minifi::processors::GetTCP::ReconnectInterval, "100 msec");
  // we're using new lines above
  context->setProperty(org::apache::nifi::minifi::processors::GetTCP::EndOfMessageByte, "10");
  auto session = utils::debug_make_shared<core::ProcessSession>(context);
    auto session2 = utils::debug_make_shared<core::ProcessSession>(context2);


  REQUIRE(processor->getName() == "gettcpexample");

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::FlowFile> record;
  processor->setScheduledState(core::ScheduledState::RUNNING);

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessSessionFactory> factory = utils::debug_make_shared<core::ProcessSessionFactory>(context);
  processor->onSchedule(context, factory);
  processor->onTrigger(context, session);
  server.writeData(buffer, buffer.size());
  std::this_thread::sleep_for(std::chrono::seconds(2));

  logAttribute->initialize();
  logAttribute->incrementActiveTasks();
  logAttribute->setScheduledState(core::ScheduledState::RUNNING);
  org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessSessionFactory> factory2 = utils::debug_make_shared<core::ProcessSessionFactory>(context2);
  logAttribute->onSchedule(context2, factory2);
  logAttribute->onTrigger(context2, session2);

  auto reporter = session->getProvenanceReporter();
  auto records = reporter->getEvents();
  record = session->get();
  REQUIRE(record == nullptr);
  REQUIRE(records.size() == 0);

  processor->incrementActiveTasks();
  processor->setScheduledState(core::ScheduledState::RUNNING);
  processor->onTrigger(context, session);
  reporter = session->getProvenanceReporter();

  records = reporter->getEvents();
  session->commit();

  logAttribute->incrementActiveTasks();
  logAttribute->setScheduledState(core::ScheduledState::RUNNING);
  logAttribute->onTrigger(context2, session2);

  records = reporter->getEvents();

  logAttribute->incrementActiveTasks();
  logAttribute->setScheduledState(core::ScheduledState::RUNNING);
  logAttribute->onTrigger(context2, session2);

  records = reporter->getEvents();

  REQUIRE(true == LogTestController::getInstance().contains("Size:2 Offset:0"));
  LogTestController::getInstance().reset();
}

TEST_CASE("GetTCPEmptyNoConnect", "[GetTCP3]") {
  TestController testController;
  LogTestController::getInstance().setDebug<minifi::processors::LogAttribute>();
  LogTestController::getInstance().setDebug<minifi::processors::GetTCP>();
  LogTestController::getInstance().setTrace<minifi::io::Socket>();

  org::apache::nifi::minifi::utils::debug_shared_ptr<TestPlan> plan = testController.createPlan();
  org::apache::nifi::minifi::utils::debug_shared_ptr<core::Processor> getfile = plan->addProcessor("GetTCP", "gettcpexample");

  plan->addProcessor("LogAttribute", "logattribute", core::Relationship("success", "description"), true);

  plan->setProperty(getfile, org::apache::nifi::minifi::processors::GetTCP::EndpointList.getName(), org::apache::nifi::minifi::io::Socket::getMyHostName() + ":9182");
  plan->setProperty(getfile, org::apache::nifi::minifi::processors::GetTCP::ReconnectInterval.getName(), "100 msec");
  // we're using new lines above
  plan->setProperty(getfile, org::apache::nifi::minifi::processors::GetTCP::EndOfMessageByte.getName(), "10");

  testController.runSession(plan, false);
  auto records = plan->getProvenanceRecords();
  org::apache::nifi::minifi::utils::debug_shared_ptr<core::FlowFile> record = plan->getCurrentFlowFile();
  REQUIRE(record == nullptr);
  REQUIRE(records.size() == 0);

  REQUIRE(true == LogTestController::getInstance().contains("Could not create socket during initialization for " + org::apache::nifi::minifi::io::Socket::getMyHostName()  + ":9182"));
  LogTestController::getInstance().reset();
}





