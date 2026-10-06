#include <gtest/gtest.h>

#include <fstream>
#include <string>
#include <vector>

#include "desk/protocol/wire.hpp"

using namespace desk;
using namespace desk::protocol;

namespace {

Frame sample() {
  Frame frame;
  frame.secondsOfDay = 13 * 3600 + 7 * 60;
  frame.nightStart = 22;
  frame.nightEnd = 7;
  frame.settings.swapButtons = true;
  frame.settings.dayBrightness = 55;
  domain::Session a;
  a.id = domain::SessionId{0xDEADBEEF};
  a.agent = domain::Agent::Codex;
  a.state = domain::SessionState::NeedsYou;
  a.subagents = 3;
  a.inState = Millis{42'000};
  a.name = "Миграция базы";
  a.detail = "perm";
  frame.snapshot.sessions.push_back(a);
  domain::Session b;
  b.id = domain::SessionId{7};
  b.name = "desksessions-d9";
  frame.snapshot.sessions.push_back(b);
  return frame;
}

std::vector<std::vector<std::uint8_t>> chunk(std::span<const std::uint8_t> frame, std::uint8_t seq,
                                             std::size_t size) {
  std::vector<std::vector<std::uint8_t>> chunks;
  const std::size_t payload = size - kChunkHeaderBytes;
  const auto count = static_cast<std::uint8_t>((frame.size() + payload - 1) / payload);
  for (std::uint8_t i = 0; i < count; ++i) {
    std::vector<std::uint8_t> c{seq, i, count};
    const auto slice = frame.subspan(i * payload, std::min(payload, frame.size() - i * payload));
    c.insert(c.end(), slice.begin(), slice.end());
    chunks.push_back(c);
  }
  return chunks;
}

}  // namespace

TEST(Wire, FrameRoundTrips) {
  std::array<std::uint8_t, kMaxFrameBytes> buffer{};
  const std::size_t n = encodeFrame(sample(), buffer);
  ASSERT_GT(n, 0u);
  const auto decoded = decodeFrame(std::span(buffer.data(), n));
  ASSERT_TRUE(decoded.has_value());
  EXPECT_EQ(decoded->secondsOfDay, 13u * 3600 + 7 * 60);
  EXPECT_EQ(decoded->nightStart, 22);
  EXPECT_FALSE(decoded->settings.flipScreen);
  EXPECT_TRUE(decoded->settings.swapButtons);
  EXPECT_EQ(decoded->settings.dayBrightness, 55);
  ASSERT_EQ(decoded->snapshot.sessions.size(), 2u);
  const domain::Session& a = decoded->snapshot.sessions[0];
  EXPECT_EQ(a.id.value, 0xDEADBEEFu);
  EXPECT_EQ(a.agent, domain::Agent::Codex);
  EXPECT_EQ(a.state, domain::SessionState::NeedsYou);
  EXPECT_EQ(a.subagents, 3);
  EXPECT_EQ(a.inState, Millis{42'000});
  EXPECT_STREQ(a.name.c_str(), "Миграция базы");
  EXPECT_STREQ(a.detail.c_str(), "perm");
}

TEST(Wire, RejectsTruncatedAndUnknownFrames) {
  std::array<std::uint8_t, kMaxFrameBytes> buffer{};
  const std::size_t n = encodeFrame(sample(), buffer);
  EXPECT_FALSE(decodeFrame(std::span(buffer.data(), n - 1)).has_value());
  buffer[0] = 2;
  EXPECT_FALSE(decodeFrame(std::span(buffer.data(), n)).has_value());
}

TEST(Wire, RejectsBadEnums) {
  std::array<std::uint8_t, kMaxFrameBytes> buffer{};
  const std::size_t n = encodeFrame(sample(), buffer);
  buffer[kFrameHeaderBytes + 5] = 9;  // first session's state
  EXPECT_FALSE(decodeFrame(std::span(buffer.data(), n)).has_value());
}

TEST(Wire, ReassemblesChunksAndDropsInterruptedFrames) {
  std::array<std::uint8_t, kMaxFrameBytes> buffer{};
  const std::size_t n = encodeFrame(sample(), buffer);
  const auto chunks = chunk(std::span(buffer.data(), n), 4, 20);
  ASSERT_GT(chunks.size(), 2u);

  Reassembler reassembler;
  // A new frame interrupts this one halfway.
  reassembler.push(chunks[0]);
  auto interrupted = chunk(std::span(buffer.data(), n), 5, 20);
  std::optional<std::span<const std::uint8_t>> done;
  for (const auto& c : interrupted) {
    done = reassembler.push(c);
  }
  ASSERT_TRUE(done.has_value());
  EXPECT_TRUE(decodeFrame(*done).has_value());

  // Out-of-order chunks are rejected.
  EXPECT_FALSE(reassembler.push(chunks[1]).has_value());
}

TEST(Wire, CommandsRoundTrip) {
  const domain::Command command{.kind = domain::Command::Kind::Jump,
                                .session = domain::SessionId{0x01020304}};
  const auto bytes = encodeCommand(command);
  EXPECT_EQ(bytes[0], 1);
  EXPECT_EQ(bytes[1], 0x04);
  EXPECT_EQ(decodeCommand(bytes), command);
}

TEST(Wire, TruncatesNamesOnUtf8Boundaries) {
  Frame frame;
  domain::Session s;
  // 47 ASCII bytes then a two-byte letter: cutting at 48 would split it.
  s.name.assign(47, 'a');
  frame.snapshot.sessions.push_back(s);
  frame.snapshot.sessions[0].name.append("ж");  // etl::string<48> keeps 48 bytes: 47 + 1 half
  std::array<std::uint8_t, kMaxFrameBytes> buffer{};
  const std::size_t n = encodeFrame(frame, buffer);
  const auto decoded = decodeFrame(std::span(buffer.data(), n));
  ASSERT_TRUE(decoded.has_value());
  EXPECT_EQ(decoded->snapshot.sessions[0].name.size(), 47u);
}

TEST(Wire, FullFrameFitsTheChunkBudget) {
  EXPECT_LE(kMaxFrameBytes, kMaxChunks * (182 - kChunkHeaderBytes));
}

TEST(Wire, DecodesTheGoldenFrameTheMacEncodes) {
  // protocol/golden/frame-v1.hex is also asserted byte-for-byte by the Mac app's tests.
  // Tests run from the firmware directory.
  const std::string path = "../protocol/golden/frame-v1.hex";
  std::ifstream file(path);
  ASSERT_TRUE(file.good()) << path;
  std::string hex;
  file >> hex;
  std::vector<std::uint8_t> bytes;
  for (std::size_t i = 0; i + 1 < hex.size(); i += 2) {
    bytes.push_back(static_cast<std::uint8_t>(std::stoi(hex.substr(i, 2), nullptr, 16)));
  }

  const auto frame = decodeFrame(bytes);
  ASSERT_TRUE(frame.has_value());
  EXPECT_EQ(frame->secondsOfDay, 12u * 3600 + 34 * 60 + 56);
  EXPECT_EQ(frame->nightStart, 23);
  EXPECT_EQ(frame->nightEnd, 8);
  EXPECT_FALSE(frame->settings.flipScreen);
  EXPECT_TRUE(frame->settings.swapButtons);
  EXPECT_EQ(frame->settings.dayBrightness, 80);
  EXPECT_EQ(frame->settings.nightBrightness, 12);
  ASSERT_EQ(frame->snapshot.sessions.size(), 2u);
  const auto& a = frame->snapshot.sessions[0];
  EXPECT_EQ(a.id.value, 0x01020304u);
  EXPECT_EQ(a.state, domain::SessionState::NeedsYou);
  EXPECT_EQ(a.inState, Millis{42'000});
  EXPECT_STREQ(a.name.c_str(), "servarium-05");
  EXPECT_STREQ(a.detail.c_str(), "perm");
  const auto& b = frame->snapshot.sessions[1];
  EXPECT_EQ(b.agent, domain::Agent::Codex);
  EXPECT_EQ(b.subagents, 3);
  EXPECT_STREQ(b.name.c_str(), "Миграция базы");
}

int main(int argc, char** argv) {
  ::testing::InitGoogleTest(&argc, argv);
  return RUN_ALL_TESTS();
}
