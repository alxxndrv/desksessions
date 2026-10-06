#pragma once

#include <etl/vector.h>

#include <cstddef>
#include <optional>
#include <span>

#include "desk/anim/channel.hpp"
#include "desk/domain/session_board.hpp"
#include "desk/util/time.hpp"

namespace desk::scene {

/// One on-screen row and everything about it that moves. Slots outlive their sessions briefly so
/// a removed row can slide away, and a row that comes back mid-exit reverses instead of popping.
struct RowSlot {
  domain::Session session;
  domain::SessionState previousState = domain::SessionState::Working;
  bool exiting = false;
  Instant stateSince;  // when this row entered its current state, on the local clock

  anim::Channel y;                 // top edge in list coordinates
  anim::Channel x{0.0f};           // horizontal offset: enter, exit, refusal wiggle
  anim::Channel opacity;           // 0 → 1
  anim::Channel lift;              // 0 → 1 brightening while held or reordering
  anim::Channel stateBlend;        // 0 → 1 crossfade from previousState's colour
  anim::Channel scale{1.0f};       // long-press pop
  anim::Channel glyphScale{1.0f};  // DONE check pop

  void update(Instant now);
};

/// The animated picture of a SessionBoard: retained motion state, rebuilt from the board by
/// diffing on session id. Holds no drawing code; the renderer reads it every frame.
class BoardScene {
 public:
  static constexpr std::size_t kSlots = domain::kMaxSessions + 4;

  /// Reconciles rows with the board after a snapshot, a dismissal or a cursor reset.
  void sync(const domain::SessionBoard& board, Instant now);

  /// The cursor moved on `board` (already updated).
  void cursorMoved(const domain::SessionBoard& board, domain::CursorMoved moved, Instant now);

  /// Press feedback: the list leans 5 px toward the button (-1 = up/BOOT, +1 = down/Aux).
  void nudge(float direction, Instant now);

  /// A long press is filling (`progress` in [0, 1]) or was let go early (nullopt).
  void hold(std::optional<float> progress, Instant now);

  /// The long press completed: the selected row pops.
  void longPressFired(Instant now);

  /// The selected row can't do what was asked: it wiggles.
  void refuse(Instant now);

  void update(Instant now);

  [[nodiscard]] std::span<const RowSlot> slots() const { return slots_; }
  [[nodiscard]] float highlightY() const { return highlight_.value(); }
  [[nodiscard]] float highlightOpacity() const { return highlightOpacity_.value(); }
  [[nodiscard]] float scroll() const { return scroll_.value(); }
  [[nodiscard]] float nudgeOffset() const { return nudge_.value(); }
  [[nodiscard]] float holdFill() const { return holdFill_.value(); }
  [[nodiscard]] std::optional<domain::SessionId> selectedId() const { return selected_; }
  [[nodiscard]] Instant selectedSince() const { return selectedSince_; }
  [[nodiscard]] float count(domain::SessionState state) const;
  [[nodiscard]] float contentHeight() const;

 private:
  RowSlot* find(domain::SessionId id);
  RowSlot* selectedSlot();
  void enter(const domain::Session& session, float y, Instant now);
  void follow(const domain::SessionBoard& board, Instant now);
  void scrollToReveal(float top, Instant now);

  etl::vector<RowSlot, kSlots> slots_;
  anim::Channel highlight_;
  anim::Channel highlightOpacity_;
  anim::Channel scroll_;
  anim::Channel nudge_;
  anim::Channel holdFill_;
  anim::Channel needsYou_;
  anim::Channel done_;
  anim::Channel working_;
  std::size_t visibleRows_ = 0;
  std::optional<domain::SessionId> selected_;
  Instant selectedSince_;
  bool holding_ = false;
};

}  // namespace desk::scene
