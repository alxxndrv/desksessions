#pragma once

#include <vector>

#include "desk/app/ports.hpp"

namespace desk::test {

/// Records every command the app sends to the Mac.
class RecordingLink final : public app::CommandLink {
 public:
  void send(const domain::Command& command) override { sent.push_back(command); }
  std::vector<domain::Command> sent;
};

}  // namespace desk::test
