#include "desk/protocol/wire.hpp"

#include <algorithm>

namespace desk::protocol {
namespace {

/// Bounds-checked little-endian reader.
class Reader {
 public:
  explicit Reader(std::span<const std::uint8_t> bytes) : bytes_(bytes) {}

  bool u8(std::uint8_t& out) {
    if (pos_ + 1 > bytes_.size()) {
      return false;
    }
    out = bytes_[pos_++];
    return true;
  }
  bool u32(std::uint32_t& out) {
    if (pos_ + 4 > bytes_.size()) {
      return false;
    }
    out = static_cast<std::uint32_t>(bytes_[pos_]) |
          static_cast<std::uint32_t>(bytes_[pos_ + 1]) << 8 |
          static_cast<std::uint32_t>(bytes_[pos_ + 2]) << 16 |
          static_cast<std::uint32_t>(bytes_[pos_ + 3]) << 24;
    pos_ += 4;
    return true;
  }
  template <std::size_t N>
  bool string(etl::string<N>& out) {
    std::uint8_t length = 0;
    if (!u8(length) || length > N || pos_ + length > bytes_.size()) {
      return false;
    }
    out.assign(reinterpret_cast<const char*>(bytes_.data() + pos_), length);
    pos_ += length;
    return true;
  }
  [[nodiscard]] bool done() const { return pos_ == bytes_.size(); }

 private:
  std::span<const std::uint8_t> bytes_;
  std::size_t pos_ = 0;
};

/// Longest prefix of `text[0, n)` that doesn't end inside a UTF-8 sequence.
std::size_t completeUtf8Prefix(const char* text, std::size_t n) {
  if (n == 0) {
    return 0;
  }
  std::size_t start = n - 1;
  while (start > 0 && (static_cast<std::uint8_t>(text[start]) & 0xC0) == 0x80) {
    --start;
  }
  const auto lead = static_cast<std::uint8_t>(text[start]);
  const std::size_t need = lead < 0x80 ? 1 : lead >= 0xF0 ? 4 : lead >= 0xE0 ? 3 : 2;
  return start + need <= n ? n : start;
}

/// Little-endian writer that stops writing (and reports failure) when out of room.
class Writer {
 public:
  explicit Writer(std::span<std::uint8_t> out) : out_(out) {}

  void u8(std::uint8_t v) {
    if (pos_ < out_.size()) {
      out_[pos_] = v;
    } else {
      overflow_ = true;
    }
    ++pos_;
  }
  void u32(std::uint32_t v) {
    for (int shift = 0; shift < 32; shift += 8) {
      u8(static_cast<std::uint8_t>(v >> shift));
    }
  }
  void string(const char* text, std::size_t length, std::size_t limit) {
    const std::size_t n = completeUtf8Prefix(text, std::min(length, limit));
    u8(static_cast<std::uint8_t>(n));
    for (std::size_t i = 0; i < n; ++i) {
      u8(static_cast<std::uint8_t>(text[i]));
    }
  }
  [[nodiscard]] std::size_t written() const { return overflow_ ? 0 : pos_; }

 private:
  std::span<std::uint8_t> out_;
  std::size_t pos_ = 0;
  bool overflow_ = false;
};

}  // namespace

std::optional<Frame> decodeFrame(std::span<const std::uint8_t> bytes) {
  Reader in(bytes);
  Frame frame;
  std::uint8_t version = 0;
  std::uint8_t flags = 0;
  std::uint8_t count = 0;
  if (!in.u8(version) || version != kVersion || !in.u8(flags) || !in.u32(frame.secondsOfDay) ||
      !in.u8(frame.nightStart) || !in.u8(frame.nightEnd) || !in.u8(count) ||
      count > domain::kMaxSessions || frame.nightStart > 23 || frame.nightEnd > 23) {
    return std::nullopt;
  }
  frame.settings.flipScreen = (flags & kFlagFlipScreen) != 0;
  frame.settings.swapButtons = (flags & kFlagSwapButtons) != 0;
  for (std::uint8_t i = 0; i < count; ++i) {
    domain::Session s;
    std::uint8_t agent = 0;
    std::uint8_t state = 0;
    std::uint32_t inState = 0;
    if (!in.u32(s.id.value) || !in.u8(agent) || !in.u8(state) || !in.u8(s.subagents) ||
        !in.u32(inState) || !in.string(s.name) || !in.string(s.detail) || agent > 1 || state > 2) {
      return std::nullopt;
    }
    s.agent = static_cast<domain::Agent>(agent);
    s.state = static_cast<domain::SessionState>(state);
    s.inState = Millis{static_cast<Millis::rep>(inState) * 1000};
    frame.snapshot.sessions.push_back(s);
  }
  if (!in.done()) {
    return std::nullopt;
  }
  return frame;
}

std::size_t encodeFrame(const Frame& frame, std::span<std::uint8_t> out) {
  Writer w(out);
  w.u8(kVersion);
  w.u8(static_cast<std::uint8_t>((frame.settings.flipScreen ? kFlagFlipScreen : 0) |
                                 (frame.settings.swapButtons ? kFlagSwapButtons : 0)));
  w.u32(frame.secondsOfDay);
  w.u8(frame.nightStart);
  w.u8(frame.nightEnd);
  w.u8(static_cast<std::uint8_t>(frame.snapshot.sessions.size()));
  for (const domain::Session& s : frame.snapshot.sessions) {
    w.u32(s.id.value);
    w.u8(static_cast<std::uint8_t>(s.agent));
    w.u8(static_cast<std::uint8_t>(s.state));
    w.u8(s.subagents);
    w.u32(static_cast<std::uint32_t>(std::max<Millis::rep>(0, s.inState.count() / 1000)));
    w.string(s.name.data(), s.name.size(), domain::kMaxNameBytes);
    w.string(s.detail.data(), s.detail.size(), domain::kMaxDetailBytes);
  }
  return w.written();
}

std::array<std::uint8_t, kCommandBytes> encodeCommand(const domain::Command& command) {
  const std::uint32_t id = command.session.value;
  return {static_cast<std::uint8_t>(command.kind), static_cast<std::uint8_t>(id),
          static_cast<std::uint8_t>(id >> 8), static_cast<std::uint8_t>(id >> 16),
          static_cast<std::uint8_t>(id >> 24)};
}

std::optional<domain::Command> decodeCommand(std::span<const std::uint8_t> bytes) {
  Reader in(bytes);
  std::uint8_t kind = 0;
  domain::Command command{};
  if (!in.u8(kind) || kind > 1 || !in.u32(command.session.value) || !in.done()) {
    return std::nullopt;
  }
  command.kind = static_cast<domain::Command::Kind>(kind);
  return command;
}

std::optional<std::span<const std::uint8_t>> Reassembler::push(
    std::span<const std::uint8_t> chunk) {
  if (chunk.size() < kChunkHeaderBytes) {
    return std::nullopt;
  }
  const std::uint8_t frame = chunk[0];
  const std::uint8_t index = chunk[1];
  const std::uint8_t count = chunk[2];
  if (count == 0 || count > kMaxChunks || index >= count) {
    return std::nullopt;
  }
  if (index == 0 || frame_ != frame) {
    // A new frame starts; anything half-received is abandoned.
    buffer_.clear();
    frame_ = frame;
    expected_ = 0;
    if (index != 0) {
      frame_.reset();
      return std::nullopt;
    }
  }
  if (index != expected_) {
    frame_.reset();
    return std::nullopt;
  }
  const auto payload = chunk.subspan(kChunkHeaderBytes);
  if (buffer_.available() < payload.size()) {
    frame_.reset();
    return std::nullopt;
  }
  buffer_.insert(buffer_.end(), payload.begin(), payload.end());
  ++expected_;
  if (expected_ == count) {
    frame_.reset();
    return std::span<const std::uint8_t>(buffer_.data(), buffer_.size());
  }
  return std::nullopt;
}

}  // namespace desk::protocol
