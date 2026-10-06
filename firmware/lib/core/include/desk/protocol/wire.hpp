#pragma once

#include <etl/vector.h>

#include <array>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <span>

#include "desk/app/device_settings.hpp"
#include "desk/domain/command.hpp"
#include "desk/domain/session.hpp"

/// The BLE wire format, as specified in protocol/PROTOCOL.md.
namespace desk::protocol {

inline constexpr std::uint8_t kVersion = 1;
inline constexpr std::size_t kChunkHeaderBytes = 3;
inline constexpr std::size_t kMaxChunks = 8;
inline constexpr std::size_t kFrameHeaderBytes = 9;
inline constexpr std::size_t kMaxSessionBytes =
    4 + 1 + 1 + 1 + 4 + 1 + domain::kMaxNameBytes + 1 + domain::kMaxDetailBytes;
inline constexpr std::size_t kMaxFrameBytes =
    kFrameHeaderBytes + domain::kMaxSessions * kMaxSessionBytes;
inline constexpr std::size_t kCommandBytes = 5;

/// Everything one snapshot write carries.
struct Frame {
  domain::Snapshot snapshot;
  std::uint32_t secondsOfDay = 0;
  std::uint8_t nightStart = 23;
  std::uint8_t nightEnd = 8;
  app::DeviceSettings settings;
};

/// Bits of the frame's `flags` byte.
inline constexpr std::uint8_t kFlagFlipScreen = 1u << 0;
inline constexpr std::uint8_t kFlagSwapButtons = 1u << 1;

/// Parses a reassembled frame. Rejects unknown versions, bad enums and overruns.
[[nodiscard]] std::optional<Frame> decodeFrame(std::span<const std::uint8_t> bytes);

/// Serialises `frame` into `out`, truncating strings on UTF-8 boundaries. Returns bytes written.
[[nodiscard]] std::size_t encodeFrame(const Frame& frame, std::span<std::uint8_t> out);

[[nodiscard]] std::array<std::uint8_t, kCommandBytes> encodeCommand(const domain::Command& command);
[[nodiscard]] std::optional<domain::Command> decodeCommand(std::span<const std::uint8_t> bytes);

/// Collects chunks back into a frame.
class Reassembler {
 public:
  /// Feeds one chunk. Returns the whole frame once its last chunk arrives; the view stays valid
  /// until the next push.
  std::optional<std::span<const std::uint8_t>> push(std::span<const std::uint8_t> chunk);

 private:
  etl::vector<std::uint8_t, kMaxFrameBytes> buffer_;
  std::optional<std::uint8_t> frame_;
  std::uint8_t expected_ = 0;
};

}  // namespace desk::protocol
