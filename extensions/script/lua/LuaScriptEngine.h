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

#ifndef NIFI_MINIFI_CPP_LUASCRIPTENGINE_H
#define NIFI_MINIFI_CPP_LUASCRIPTENGINE_H

#include <mutex>
#include <sol.hpp>
#include <core/ProcessSession.h>

#include "../ScriptEngine.h"
#include "../ScriptProcessContext.h"

#include "LuaProcessSession.h"

namespace org {
namespace apache {
namespace nifi {
namespace minifi {
namespace lua {

class LuaScriptEngine : public script::ScriptEngine {
 public:
  LuaScriptEngine();

  void eval(const std::string &script) override;
  void evalFile(const std::string &file_name) override;

  /**
   * Calls the given function, forwarding arbitrary provided parameters.
   *
   * @return
   */
  template<typename... Args>
  void call(const std::string &fn_name, Args &&...args) {
    sol::function fn = lua_[fn_name.c_str()];
    fn(convert(args)...);
  }

  class TriggerSession {
   public:
    TriggerSession(org::apache::nifi::minifi::utils::debug_shared_ptr<script::ScriptProcessContext> script_context,
                   org::apache::nifi::minifi::utils::debug_shared_ptr<lua::LuaProcessSession> lua_session)
        : script_context_(std::move(script_context)),
          lua_session_(std::move(lua_session)) {
    }

    ~TriggerSession() {
      script_context_->releaseProcessContext();
      lua_session_->releaseCoreResources();
    }


   private:
    org::apache::nifi::minifi::utils::debug_shared_ptr<script::ScriptProcessContext> script_context_;
    org::apache::nifi::minifi::utils::debug_shared_ptr<LuaProcessSession> lua_session_;
  };

  void onTrigger(const org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessContext> &context,
      const org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessSession> &session) {
    auto script_context = convert(context);
    auto lua_session = convert(session);
    TriggerSession trigger_session(script_context, lua_session);
    call("onTrigger", script_context, lua_session);
  }

  template<typename T>
  void bind(const std::string &name, const T &value) {
    lua_[name.c_str()] = convert(value);
  }

  template<typename T>
  T convert(const T &value) {
    return value;
  }

  org::apache::nifi::minifi::utils::debug_shared_ptr<script::ScriptProcessContext> convert(const org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessContext> &context) {
    return utils::debug_make_shared<script::ScriptProcessContext>(context);
  }

  org::apache::nifi::minifi::utils::debug_shared_ptr<LuaProcessSession> convert(const org::apache::nifi::minifi::utils::debug_shared_ptr<core::ProcessSession> &session) {
    return utils::debug_make_shared<LuaProcessSession>(session);
  }

 private:
  sol::state lua_;
};

} /* namespace lua */
} /* namespace minifi */
} /* namespace nifi */
} /* namespace apache */
} /* namespace org */

#endif //NIFI_MINIFI_CPP_LUASCRIPTENGINE_H
