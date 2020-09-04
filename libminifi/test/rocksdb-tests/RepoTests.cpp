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

#include <chrono>
#include <map>
#include <memory>
#include <string>
#include <thread>

#include "core/Core.h"
#include "core/repository/AtomicRepoEntries.h"
#include "core/RepositoryFactory.h"
#include "FlowFileRecord.h"
#include "FlowFileRepository.h"
#include "provenance/Provenance.h"
#include "properties/Configure.h"
#include "../unit/ProvenanceTestHelper.h"
#include "../TestBase.h"

TEST_CASE("Test Repo Names", "[TestFFR1]") {
  auto repoA = minifi::core::createRepository("FlowFileRepository", false, "flowfile");
  REQUIRE("flowfile" == repoA->getName());

  auto repoB = minifi::core::createRepository("ProvenanceRepository", false, "provenance");
  REQUIRE("provenance" == repoB->getName());
}

TEST_CASE("Test Repo Empty Value Attribute", "[TestFFR1]") {
  LogTestController::getInstance().setDebug<core::ContentRepository>();
  LogTestController::getInstance().setDebug<core::repository::FileSystemRepository>();
  LogTestController::getInstance().setDebug<core::repository::FlowFileRepository>();
  TestController testController;
  char format[] = "/var/tmp/testRepo.XXXXXX";
  auto dir = testController.createTempDirectory(format);
  org::apache::nifi::minifi::utils::debug_shared_ptr<core::repository::FlowFileRepository> repository = org::apache::nifi::minifi::utils::debug_make_shared<core::repository::FlowFileRepository>("ff", dir, 0, 0, 1);

  repository->initialize(org::apache::nifi::minifi::utils::debug_make_shared<minifi::Configure>());

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::ContentRepository> content_repo = org::apache::nifi::minifi::utils::debug_make_shared<core::repository::VolatileContentRepository>();
  minifi::FlowFileRecord record(repository, content_repo);

  record.addAttribute("keyA", "");

  REQUIRE(true == record.Serialize());

  utils::file::FileUtils::delete_dir(FLOWFILE_CHECKPOINT_DIRECTORY, true);

  repository->stop();
}

TEST_CASE("Test Repo Empty Key Attribute ", "[TestFFR2]") {
  LogTestController::getInstance().setDebug<core::ContentRepository>();
  LogTestController::getInstance().setDebug<core::repository::FileSystemRepository>();
  LogTestController::getInstance().setDebug<core::repository::FlowFileRepository>();
  TestController testController;
  char format[] = "/var/tmp/testRepo.XXXXXX";
  auto dir = testController.createTempDirectory(format);
  org::apache::nifi::minifi::utils::debug_shared_ptr<core::repository::FlowFileRepository> repository = org::apache::nifi::minifi::utils::debug_make_shared<core::repository::FlowFileRepository>("ff", dir, 0, 0, 1);

  repository->initialize(org::apache::nifi::minifi::utils::debug_make_shared<minifi::Configure>());
  org::apache::nifi::minifi::utils::debug_shared_ptr<core::ContentRepository> content_repo = org::apache::nifi::minifi::utils::debug_make_shared<core::repository::VolatileContentRepository>();
  minifi::FlowFileRecord record(repository, content_repo);

  record.addAttribute("keyA", "hasdgasdgjsdgasgdsgsadaskgasd");

  record.addAttribute("", "hasdgasdgjsdgasgdsgsadaskgasd");

  REQUIRE(true == record.Serialize());

  utils::file::FileUtils::delete_dir(FLOWFILE_CHECKPOINT_DIRECTORY, true);

  repository->stop();
}

TEST_CASE("Test Repo Key Attribute Verify ", "[TestFFR3]") {
  LogTestController::getInstance().setDebug<core::ContentRepository>();
  LogTestController::getInstance().setDebug<core::repository::FileSystemRepository>();
  LogTestController::getInstance().setDebug<core::repository::FlowFileRepository>();
  TestController testController;
  char format[] = "/var/tmp/testRepo.XXXXXX";
  auto dir = testController.createTempDirectory(format);
  org::apache::nifi::minifi::utils::debug_shared_ptr<core::repository::FlowFileRepository> repository = org::apache::nifi::minifi::utils::debug_make_shared<core::repository::FlowFileRepository>("ff", dir, 0, 0, 1);

  repository->initialize(org::apache::nifi::minifi::utils::debug_make_shared<org::apache::nifi::minifi::Configure>());

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::ContentRepository> content_repo = org::apache::nifi::minifi::utils::debug_make_shared<core::repository::VolatileContentRepository>();
  minifi::FlowFileRecord record(repository, content_repo);

  minifi::FlowFileRecord record2(repository, content_repo);

  std::string uuid = record.getUUIDStr();

  record.addAttribute("keyA", "hasdgasdgjsdgasgdsgsadaskgasd");

  record.addAttribute("keyB", "");

  record.addAttribute("", "");

  record.updateAttribute("", "hasdgasdgjsdgasgdsgsadaskgasd2");

  record.addAttribute("", "sdgsdg");

  REQUIRE(record.Serialize());

  repository->stop();

  record2.DeSerialize(uuid);

  std::string value;
  REQUIRE(record2.getAttribute("", value));

  REQUIRE("hasdgasdgjsdgasgdsgsadaskgasd2" == value);

  REQUIRE(!record2.getAttribute("key", value));
  REQUIRE(record2.getAttribute("keyA", value));
  REQUIRE("hasdgasdgjsdgasgdsgsadaskgasd" == value);

  REQUIRE(record2.getAttribute("keyB", value));
  REQUIRE(value.empty());

  utils::file::FileUtils::delete_dir(FLOWFILE_CHECKPOINT_DIRECTORY, true);
}

TEST_CASE("Test Delete Content ", "[TestFFR4]") {
  TestController testController;
  char format[] = "/var/tmp/testRepo.XXXXXX";
  LogTestController::getInstance().setDebug<core::ContentRepository>();
  LogTestController::getInstance().setDebug<core::repository::FileSystemRepository>();
  LogTestController::getInstance().setDebug<core::repository::FlowFileRepository>();

  auto dir = testController.createTempDirectory(format);

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::repository::FlowFileRepository> repository = org::apache::nifi::minifi::utils::debug_make_shared<core::repository::FlowFileRepository>("ff", dir, 0, 0, 1);

  std::map<std::string, std::string> attributes;

  std::fstream file;
  std::stringstream ss;
  ss << dir << utils::file::FileUtils::get_separator() << "tstFile.ext";
  file.open(ss.str(), std::ios::out);
  file << "tempFile";
  file.close();

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::ContentRepository> content_repo = org::apache::nifi::minifi::utils::debug_make_shared<core::repository::FileSystemRepository>();

  repository->initialize(org::apache::nifi::minifi::utils::debug_make_shared<minifi::Configure>());

  repository->loadComponent(content_repo);

  org::apache::nifi::minifi::utils::debug_shared_ptr<minifi::ResourceClaim> claim = org::apache::nifi::minifi::utils::debug_make_shared<minifi::ResourceClaim>(ss.str(), content_repo);

  {
    minifi::FlowFileRecord record(repository, content_repo, attributes, claim);

    record.addAttribute("keyA", "hasdgasdgjsdgasgdsgsadaskgasd");

    record.addAttribute("", "hasdgasdgjsdgasgdsgsadaskgasd");

    REQUIRE(record.Serialize());

    REQUIRE(repository->Delete(record.getUUIDStr()));
    claim->decreaseFlowFileRecordOwnedCount();

    repository->flush();

    repository->stop();
  }

  std::ifstream fileopen(ss.str(), std::ios::in);
  REQUIRE(!fileopen.good());

  utils::file::FileUtils::delete_dir(FLOWFILE_CHECKPOINT_DIRECTORY, true);

  LogTestController::getInstance().reset();
}

TEST_CASE("Test Validate Checkpoint ", "[TestFFR5]") {
  TestController testController;
  utils::file::FileUtils::delete_dir(FLOWFILE_CHECKPOINT_DIRECTORY, true);
  char format[] = "/var/tmp/testRepo.XXXXXX";
  LogTestController::getInstance().setDebug<core::ContentRepository>();
  LogTestController::getInstance().setTrace<core::repository::FileSystemRepository>();
  LogTestController::getInstance().setTrace<minifi::ResourceClaim>();
  LogTestController::getInstance().setTrace<minifi::FlowFileRecord>();

  auto dir = testController.createTempDirectory(format);

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::repository::FlowFileRepository> repository = org::apache::nifi::minifi::utils::debug_make_shared<core::repository::FlowFileRepository>("ff", dir, 0, 0, 1);

  std::map<std::string, std::string> attributes;

  std::fstream file;
  std::stringstream ss;
  ss << dir << utils::file::FileUtils::get_separator() << "tstFile.ext";
  file.open(ss.str(), std::ios::out);
  file << "tempFile";
  file.close();

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::ContentRepository> content_repo = org::apache::nifi::minifi::utils::debug_make_shared<core::repository::FileSystemRepository>();

  repository->initialize(org::apache::nifi::minifi::utils::debug_make_shared<minifi::Configure>());

  repository->loadComponent(content_repo);

  org::apache::nifi::minifi::utils::debug_shared_ptr<minifi::ResourceClaim> claim = org::apache::nifi::minifi::utils::debug_make_shared<minifi::ResourceClaim>(ss.str(), content_repo);
  {
    minifi::FlowFileRecord record(repository, content_repo, attributes, claim);

    record.addAttribute("keyA", "hasdgasdgjsdgasgdsgsadaskgasd");

    record.addAttribute("", "hasdgasdgjsdgasgdsgsadaskgasd");

    REQUIRE(record.Serialize());

    repository->flush();

    repository->stop();

    repository->loadComponent(content_repo);

    repository->start();

    std::this_thread::sleep_for(std::chrono::milliseconds(500));
    repository->stop();
    claim = nullptr;
    // sleep for 100 ms to let the delete work.

    std::this_thread::sleep_for(std::chrono::milliseconds(500));
  }

  std::ifstream fileopen(ss.str(), std::ios::in);
  REQUIRE(fileopen.fail());

  utils::file::FileUtils::delete_dir(FLOWFILE_CHECKPOINT_DIRECTORY, true);

  LogTestController::getInstance().reset();
}

TEST_CASE("Test FlowFile Restore", "[TestFFR6]") {
  TestController testController;
  LogTestController::getInstance().setDebug<core::ContentRepository>();
  LogTestController::getInstance().setTrace<core::repository::FileSystemRepository>();
  LogTestController::getInstance().setTrace<minifi::ResourceClaim>();
  LogTestController::getInstance().setTrace<minifi::FlowFileRecord>();

  char format[] = "/var/tmp/testRepo.XXXXXX";
  auto dir = testController.createTempDirectory(format);

  auto config = org::apache::nifi::minifi::utils::debug_make_shared<minifi::Configure>();
  config->set(minifi::Configure::nifi_dbcontent_repository_directory_default, utils::file::FileUtils::concat_path(dir, "content_repository"));
  config->set(minifi::Configure::nifi_flowfile_repository_directory_default, utils::file::FileUtils::concat_path(dir, "flowfile_repository"));

  org::apache::nifi::minifi::utils::debug_shared_ptr<core::Repository> prov_repo = org::apache::nifi::minifi::utils::debug_make_shared<TestRepository>();
  org::apache::nifi::minifi::utils::debug_shared_ptr<core::repository::FlowFileRepository> ff_repository = org::apache::nifi::minifi::utils::debug_make_shared<core::repository::FlowFileRepository>("flowFileRepository");
  org::apache::nifi::minifi::utils::debug_shared_ptr<core::ContentRepository> content_repo = org::apache::nifi::minifi::utils::debug_make_shared<core::repository::FileSystemRepository>();
  ff_repository->initialize(config);
  content_repo->initialize(config);

  core::Relationship inputRel{"Input", "dummy"};
  org::apache::nifi::minifi::utils::debug_shared_ptr<minifi::Connection> input = org::apache::nifi::minifi::utils::debug_make_shared<minifi::Connection>(ff_repository, content_repo, "Input");
  input->setRelationship(inputRel);

  auto root = org::apache::nifi::minifi::utils::debug_make_shared<core::ProcessGroup>(core::ProcessGroupType::ROOT_PROCESS_GROUP, "root");
  root->addConnection(input);

  auto flowConfig = std::unique_ptr<core::FlowConfiguration>{new core::FlowConfiguration(prov_repo, ff_repository, content_repo, nullptr, config, "")};
  auto flowController = org::apache::nifi::minifi::utils::debug_make_shared<minifi::FlowController>(prov_repo, ff_repository, config, std::move(flowConfig), content_repo, "", true);

  std::string data = "banana";
  minifi::io::DataStream content(reinterpret_cast<const uint8_t*>(data.c_str()), data.length());

  /**
   * Currently it is the Connection's responsibility to persist the incoming
   * flowFiles to the FlowFileRepository. Upon restart the FlowFileRepository
   * checks the persisted database and moves every FlowFile into the Connection
   * that persisted it (if it can find it. We could have a different flow, in
   * which case the orphan FlowFiles are deleted.)
   */
  {
    org::apache::nifi::minifi::utils::debug_shared_ptr<core::Processor> processor = org::apache::nifi::minifi::utils::debug_make_shared<core::Processor>("dummy");
    utils::Identifier uuid;
    REQUIRE(processor->getUUID(uuid));
    input->setSourceUUID(uuid);
    processor->addConnection(input);
    org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessorNode> node = org::apache::nifi::minifi::utils::debug_make_shared<core::ProcessorNode>(processor);
    auto context = org::apache::nifi::minifi::utils::debug_make_shared<core::ProcessContext>(node, nullptr, prov_repo, ff_repository, content_repo);
    core::ProcessSession sessionGenFlowFile(context);
    org::apache::nifi::minifi::utils::debug_shared_ptr<core::FlowFile> flow = static_pointer_cast<core::FlowFile>(sessionGenFlowFile.create());
    sessionGenFlowFile.importFrom(content, flow);
    sessionGenFlowFile.transfer(flow, inputRel);
    sessionGenFlowFile.commit();
  }

  // remove flow from the connection but it is still present in the
  // flowFileRepo
  std::set<org::apache::nifi::minifi::utils::debug_shared_ptr<core::FlowFile>> expiredFiles;
  auto oldFlow = input->poll(expiredFiles);
  REQUIRE(oldFlow);
  REQUIRE(expiredFiles.empty());

  // this notifies the FlowFileRepository of the flow structure
  // i.e. what Connections are present (more precisely what Connectables
  // are present)
  flowController->load(root);
  // this will first check the persisted repo and restore all FlowFiles
  // that still has an owner Connectable
  ff_repository->start();

  std::this_thread::sleep_for(std::chrono::milliseconds{500});

  // check if the @input Connection's FlowFile was restored
  // upon the FlowFileRepository's startup
  auto newFlow = input->poll(expiredFiles);
  REQUIRE(newFlow);
  REQUIRE(expiredFiles.empty());

  LogTestController::getInstance().reset();
}

TEST_CASE("Flush deleted flowfiles before shutdown", "[TestFFR7]") {
  class TestFlowFileRepository: public core::repository::FlowFileRepository{
   public:
    explicit TestFlowFileRepository(const std::string& name)
        : core::SerializableComponent(name),
          FlowFileRepository(name, FLOWFILE_REPOSITORY_DIRECTORY, MAX_FLOWFILE_REPOSITORY_ENTRY_LIFE_TIME,
                             MAX_FLOWFILE_REPOSITORY_STORAGE_SIZE, 1) {}
    void flush() override {
      FlowFileRepository::flush();
      if (onFlush_) {
        onFlush_();
      }
    }
    std::function<void()> onFlush_;
  };

  TestController testController;
  char format[] = "/var/tmp/testRepo.XXXXXX";
  auto dir = testController.createTempDirectory(format);

  auto config = org::apache::nifi::minifi::utils::debug_make_shared<minifi::Configure>();
  config->set(minifi::Configure::nifi_flowfile_repository_directory_default, utils::file::FileUtils::concat_path(dir, "flowfile_repository"));

  auto content_repo = org::apache::nifi::minifi::utils::debug_make_shared<core::repository::VolatileContentRepository>();

  auto connection = org::apache::nifi::minifi::utils::debug_make_shared<minifi::Connection>(nullptr, nullptr, "Connection");
  std::map<std::string, org::apache::nifi::minifi::utils::debug_shared_ptr<core::Connectable>> connectionMap{{connection->getUUIDStr(), connection}};
  // initialize repository
  {
    org::apache::nifi::minifi::utils::debug_shared_ptr<TestFlowFileRepository> ff_repository = org::apache::nifi::minifi::utils::debug_make_shared<TestFlowFileRepository>("flowFileRepository");

    std::atomic<int> flush_counter{0};

    std::atomic<bool> stop{false};
    std::thread shutdown{[&] {
      while (!stop.load()) {
        std::this_thread::sleep_for(std::chrono::milliseconds{10});
      }
      ff_repository->stop();
    }};

    ff_repository->onFlush_ = [&] {
      if (++flush_counter != 1) {
        return;
      }

      for (int keyIdx = 0; keyIdx < 100; ++keyIdx) {
        auto file = org::apache::nifi::minifi::utils::debug_make_shared<minifi::FlowFileRecord>(ff_repository, nullptr);
        file->setUuidConnection(connection->getUUIDStr());
        // Serialize is sync
        REQUIRE(file->Serialize());
        if (keyIdx % 2 == 0) {
          // delete every second flowFile
          REQUIRE(ff_repository->Delete(file->getUUIDStr()));
        }
      }
      stop = true;
      // wait for the shutdown thread to start waiting for the worker thread
      std::this_thread::sleep_for(std::chrono::milliseconds{100});
    };

    ff_repository->setConnectionMap(connectionMap);
    REQUIRE(ff_repository->initialize(config));
    ff_repository->loadComponent(content_repo);
    ff_repository->start();

    shutdown.join();
  }

  // check if the deleted flowfiles are indeed deleted
  {
    org::apache::nifi::minifi::utils::debug_shared_ptr<TestFlowFileRepository> ff_repository = org::apache::nifi::minifi::utils::debug_make_shared<TestFlowFileRepository>("flowFileRepository");
    ff_repository->setConnectionMap(connectionMap);
    REQUIRE(ff_repository->initialize(config));
    ff_repository->loadComponent(content_repo);
    ff_repository->start();
    std::this_thread::sleep_for(std::chrono::milliseconds{100});
    REQUIRE(connection->getQueueSize() == 50);
  }
}
