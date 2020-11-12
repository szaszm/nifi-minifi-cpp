# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements.  See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership.  The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License.  You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied.  See the License for the
# specific language governing permissions and limitations
# under the License.

include(FetchContent)
FetchContent_Declare(date_src
    GIT_REPOSITORY https://github.com/HowardHinnant/date.git
    GIT_TAG        v3.0.0  # adjust tag/branch/commit as needed
)
FetchContent_GetProperties(date_src)
if (NOT date_src_POPULATED)
    FetchContent_Populate(date_src)
    set(DATE_INCLUDE_DIR 
        $<BUILD_INTERFACE:${date_src_SOURCE_DIR}/include>
        $<INSTALL_INTERFACE:include>
    )
    add_library(date INTERFACE)
    add_library(date::date ALIAS date)
    target_sources(date INTERFACE ${DATE_INCLUDE_DIR}/date/date.h)
    target_include_directories(date INTERFACE ${DATE_INCLUDE_DIR})
    target_compile_features(date INTERFACE cxx_std_11)

    add_library(date-tz STATIC ${date_src_SOURCE_DIR}/src/tz.cpp)
    add_library(date::tz ALIAS date-tz)
    target_include_directories(date-tz PUBLIC ${DATE_INCLUDE_DIR})
    target_compile_features(date-tz PUBLIC cxx_std_11)
    target_compile_definitions(date-tz PRIVATE AUTO_DOWNLOAD=0 HAS_REMOTE_API=0)
    if (WIN32)
        target_compile_definitions(date-tz PRIVATE INSTALL=. PUBLIC USE_OS_TZDB=0)
    else()
        target_compile_definitions(date-tz PUBLIC USE_OS_TZDB=1)
    endif()
    if (NOT MSVC)
        find_package(Threads)
        target_link_libraries(date-tz PUBLIC Threads::Threads)
    endif()
endif()
