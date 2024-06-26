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

#include <string>
#include <vector>

#include "unit/Catch.h"
#include "unit/TestBase.h"
#include "unit/TestUtils.h"
#include "io/FileStream.h"
#include "utils/file/FileUtils.h"

TEST_CASE("TestFileOverWrite", "[TestFiles]") {
  TestController testController;
  const auto path = testController.createTempDirectory() / "tstFile.ext";

  std::fstream file;
  file.open(path, std::ios::out);
  file << "tempFile";
  file.close();

  minifi::io::FileStream stream(path, 0, true);
  std::vector<std::byte> readBuffer;
  readBuffer.resize(stream.size());
  REQUIRE(stream.read(readBuffer) == stream.size());

  std::byte* data = readBuffer.data();

  REQUIRE(std::string(reinterpret_cast<char*>(data), readBuffer.size()) == "tempFile");

  stream.seek(4);

  stream.write(reinterpret_cast<const uint8_t*>("file"), 4);

  stream.seek(0);

  std::vector<std::byte> verifybuffer;
  verifybuffer.resize(stream.size());
  REQUIRE(stream.read(verifybuffer) == stream.size());

  data = verifybuffer.data();

  REQUIRE(std::string(reinterpret_cast<char*>(data), verifybuffer.size()) == "tempfile");
}

TEST_CASE("TestFileBadArgumentNoChange", "[TestLoader]") {
  TestController testController;
  const auto path = testController.createTempDirectory() / "tstFile.ext";

  std::fstream file;
  file.open(path, std::ios::out);
  file << "tempFile";
  file.close();

  minifi::io::FileStream stream(path, 0, true);
  std::vector<std::byte> readBuffer;
  readBuffer.resize(stream.size());
  REQUIRE(stream.read(readBuffer) == stream.size());

  auto* data = readBuffer.data();

  REQUIRE(std::string(reinterpret_cast<char*>(data), readBuffer.size()) == "tempFile");

  stream.seek(4);

  stream.write(reinterpret_cast<const uint8_t*>("file"), 0);

  stream.seek(0);

  std::vector<std::byte> verifybuffer;
  verifybuffer.resize(stream.size());
  REQUIRE(stream.read(verifybuffer) == stream.size());

  data = verifybuffer.data();

  REQUIRE(std::string(reinterpret_cast<char*>(data), verifybuffer.size()) == "tempFile");
}

TEST_CASE("TestFileBadArgumentNoChange2", "[TestLoader]") {
  TestController testController;
  const auto path = testController.createTempDirectory() / "tstFile.ext";

  std::fstream file;
  file.open(path, std::ios::out);
  file << "tempFile";
  file.close();

  minifi::io::FileStream stream(path, 0, true);
  std::vector<std::byte> readBuffer;
  readBuffer.resize(stream.size());
  REQUIRE(stream.read(readBuffer) == stream.size());

  auto* data = readBuffer.data();

  REQUIRE(std::string(reinterpret_cast<char*>(data), readBuffer.size()) == "tempFile");

  stream.seek(4);

  stream.write(nullptr, 0);

  stream.seek(0);

  std::vector<std::byte> verifybuffer;
  verifybuffer.resize(stream.size());
  REQUIRE(stream.read(verifybuffer) == stream.size());

  data = verifybuffer.data();

  REQUIRE(std::string(reinterpret_cast<char*>(data), verifybuffer.size()) == "tempFile");
}

TEST_CASE("TestFileBadArgumentNoChange3", "[TestLoader]") {
  TestController testController;
  auto path = testController.createTempDirectory() / "tstFile.ext";

  std::fstream file;
  file.open(path, std::ios::out);
  file << "tempFile";
  file.close();

  minifi::io::FileStream stream(path, 0, true);
  std::vector<std::byte> readBuffer;
  readBuffer.resize(stream.size());
  REQUIRE(stream.read(readBuffer) == stream.size());

  auto* data = readBuffer.data();

  REQUIRE(std::string(reinterpret_cast<char*>(data), readBuffer.size()) == "tempFile");

  stream.seek(4);

  stream.write(nullptr, 0);

  stream.seek(0);

  std::vector<std::byte> verifybuffer;
  data = verifybuffer.data();

  REQUIRE(std::string(reinterpret_cast<char*>(data), verifybuffer.size()).empty());
}

TEST_CASE("TestFileBeyondEnd3", "[TestLoader]") {
  TestController testController;
  const auto path = testController.createTempDirectory() / "tstFile.ext";

  std::fstream file;
  file.open(path, std::ios::out);
  file << "tempFile";
  file.close();

  minifi::io::FileStream stream(path, 0, true);
  std::vector<std::byte> readBuffer;
  readBuffer.resize(stream.size());
  REQUIRE(stream.read(readBuffer) == stream.size());

  auto* data = readBuffer.data();

  REQUIRE(std::string(reinterpret_cast<char*>(data), readBuffer.size()) == "tempFile");

  stream.seek(0);

  std::vector<std::byte> verifybuffer;
  const auto stream_size = stream.size();
  verifybuffer.resize(stream_size);
  REQUIRE(stream.read(verifybuffer) == 8);

  data = verifybuffer.data();

  REQUIRE(std::string(reinterpret_cast<char*>(data), stream_size) == "tempFile");
}

TEST_CASE("TestFileExceedSize", "[TestLoader]") {
  TestController testController;
  auto path = testController.createTempDirectory() / "tstFile.ext";

  std::fstream file;
  file.open(path, std::ios::out);
  for (int i = 0; i < 10240; i++)
    file << "tempFile";
  file.close();

  minifi::io::FileStream stream(path, 0, true);
  std::vector<std::byte> readBuffer;
  readBuffer.resize(stream.size());
  REQUIRE(stream.read(readBuffer) == stream.size());

  stream.seek(0);

  std::vector<std::byte> verifybuffer;
  verifybuffer.resize(8192);
  for (int i = 0; i < 10; i++) {
    REQUIRE(stream.read(verifybuffer) == 8192);
  }
  REQUIRE(stream.read(verifybuffer) == 0);
  stream.seek(0);
  for (int i = 0; i < 10; i++) {
    REQUIRE(stream.read(verifybuffer) == 8192);
  }
  REQUIRE(stream.read(verifybuffer) == 0);
}

TEST_CASE("Write zero bytes") {
  TestController testController;
  const auto dir = testController.createTempDirectory();
  minifi::io::FileStream stream(dir / "test.txt", 0, true);
  REQUIRE(stream.write(nullptr, 0) == 0);
}

TEST_CASE("Read zero bytes") {
  TestController testController;
  const auto dir = testController.createTempDirectory();
  minifi::io::FileStream stream(dir / "test.txt", 0, true);
  std::array<std::byte, 1> fake_buffer{};
  REQUIRE(stream.read(std::span(fake_buffer).subspan(0, 0)) == 0);
}

TEST_CASE("Non-existing file read/write test") {
  TestController test_controller;
  const auto dir = test_controller.createTempDirectory();
  minifi::io::FileStream stream(dir / "non_existing_file.txt", 0, true);
  REQUIRE(test_controller.getLog().getInstance().contains("Error opening file", std::chrono::seconds(0)));
  REQUIRE(test_controller.getLog().getInstance().contains("No such file or directory", std::chrono::seconds(0)));
  REQUIRE(minifi::io::isError(stream.write("lorem ipsum", false)));
  REQUIRE(test_controller.getLog().getInstance().contains("Error writing to file: invalid file stream", std::chrono::seconds(0)));
  std::vector<std::byte> readBuffer;
  readBuffer.resize(1);
  stream.seek(0);
  REQUIRE(minifi::io::isError(stream.read(readBuffer)));
  REQUIRE(test_controller.getLog().getInstance().contains("Error reading from file: invalid file stream", std::chrono::seconds(0)));
}

TEST_CASE("Existing file read/write test") {
  TestController test_controller;
  const auto dir = test_controller.createTempDirectory();
  const auto path_to_existing_file = dir / "existing_file.txt";
  {
    std::ofstream outfile(path_to_existing_file);
    outfile << "lorem ipsum" << std::endl;
    outfile.close();
  }
  minifi::io::FileStream stream(path_to_existing_file, 0, true);
  REQUIRE_FALSE(test_controller.getLog().getInstance().contains("Error opening file", std::chrono::seconds(0)));
  REQUIRE_FALSE(minifi::io::isError(stream.write("dolor sit amet", false)));
  REQUIRE_FALSE(test_controller.getLog().getInstance().contains("Error writing to file", std::chrono::seconds(0)));
  std::vector<std::byte> readBuffer;
  readBuffer.resize(11);
  stream.seek(0);
  REQUIRE_FALSE(minifi::io::isError(stream.read(readBuffer)));
  REQUIRE_FALSE(test_controller.getLog().getInstance().contains("Error reading from file", std::chrono::seconds(0)));
  stream.seek(0);
}

TEST_CASE("Opening file without permission creates error logs") {
  if (minifi::test::utils::runningAsUnixRoot())
    SKIP("Cannot test insufficient permissions with root user");
  TestController test_controller;
  const auto dir = test_controller.createTempDirectory();
  const auto path_to_permissionless_file = dir / "permissionless_file.txt";
  {
    std::ofstream outfile(path_to_permissionless_file);
    outfile << "this file has been just created" << std::endl;
    outfile.close();
    utils::file::FileUtils::set_permissions(path_to_permissionless_file, 0);
  }
  minifi::io::FileStream stream(path_to_permissionless_file, 0, true);  // write_enabled, because permissionless file only means read-only on Windows
  REQUIRE(test_controller.getLog().getInstance().contains("Error opening file", std::chrono::seconds(0)));
  REQUIRE(test_controller.getLog().getInstance().contains("Permission denied", std::chrono::seconds(0)));
  utils::file::FileUtils::set_permissions(path_to_permissionless_file, 0777);
}

TEST_CASE("Readonly filestream write test") {
  TestController test_controller;
  const auto dir = test_controller.createTempDirectory();
  const auto path_to_file = dir / "file_to_seek_in.txt";
  {
    std::ofstream outfile(path_to_file);
    outfile << "lorem ipsum" << std::endl;
    outfile.close();
  }
  minifi::io::FileStream stream(path_to_file, 0, false);
  REQUIRE(minifi::io::isError(stream.write("dolor sit amet", false)));
  REQUIRE(test_controller.getLog().getInstance().contains("Error writing to file: write call on file stream failed", std::chrono::seconds(0)));
}
