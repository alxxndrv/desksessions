#pragma once

#include <cstdint>
#include <utility>
#include <vector>

#include "desk/app/ports.hpp"
#include "desk/domain/session.hpp"
#include "desk/util/time.hpp"

namespace desk::demo {

/// A pretend Mac: a list of sessions that changes on a loop and reacts to the clock's commands.
class Scenario final : public app::MacLink {
 public:
  explicit Scenario(Instant start);

  /// Advances the script. Returns true when the session list changed.
  bool advance(Instant now);

  [[nodiscard]] domain::Snapshot snapshot(Instant now) const;

  void send(const domain::Command& command) override;
  void forgetPairing() override { forgetRequested_ = true; }
  /// True once after the clock asked to forget its pairing.
  bool takeForgetRequest() { return std::exchange(forgetRequested_, false); }

  // Keyboard pokes.
  void addSession(Instant now);
  void removeLast();
  void escalateFirstWorking(Instant now);
  void toggleAuto() { auto_ = !auto_; }
  /// Empties the list (everything finished and dismissed), or brings it all back.
  void toggleEmpty();

 private:
  struct Live {
    domain::Session session;
    Instant since;
  };

  Live* find(std::uint32_t id);
  void setState(std::uint32_t id, domain::SessionState state, const char* detail, Instant now);

  std::vector<Live> sessions_;
  std::vector<Live> stash_;
  std::uint32_t nextId_ = 100;
  std::size_t step_ = 0;
  Instant stepAt_;
  bool auto_ = true;
  bool dirty_ = true;
  bool forgetRequested_ = false;
};

}  // namespace desk::demo
