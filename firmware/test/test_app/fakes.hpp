#pragma once

#include <vector>

#include "desk/app/ports.hpp"

namespace desk::test {

/// Records every command the app sends to the Mac.
class RecordingLink final : public app::MacLink {
 public:
  void send(const domain::Command& command) override { sent.push_back(command); }
  void forgetPairing() override { ++forgotPairing; }
  std::vector<domain::Command> sent;
  int forgotPairing = 0;
};

}  // namespace desk::test
