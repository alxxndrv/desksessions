// Reference dumper: runs the C++ firmware core (untouched, from ../../firmware/lib) and prints
// what the Rust port must reproduce. Built and run by `cargo xtask reference`.
//
//   desk-dump wire                    wire-format vectors
//   desk-dump curves                  animation curves, sampled every millisecond
//   desk-dump trace <sim options>     the app's animated state, frame by frame, as the simulator
//                                     would run it (same clock, cues and pretend Mac)

#include <cinttypes>
#include <cstdio>
#include <cstring>
#include <optional>
#include <string>
#include <vector>

#include "desk/anim/breathing.hpp"
#include "desk/anim/channel.hpp"
#include "desk/anim/easing.hpp"
#include "desk/anim/marquee.hpp"
#include "desk/anim/moook.hpp"
#include "desk/anim/rubber_band.hpp"
#include "desk/anim/spring.hpp"
#include "desk/app/desk_app.hpp"
#include "desk/demo/scenario.hpp"
#include "desk/protocol/wire.hpp"
#include "options.hpp"

using namespace desk;

namespace {

// ---------------------------------------------------------------------------------------------
// wire

std::string hex(std::span<const std::uint8_t> bytes) {
  std::string out;
  char b[3];
  for (std::uint8_t v : bytes) {
    std::snprintf(b, sizeof(b), "%02x", v);
    out += b;
  }
  return out.empty() ? "-" : out;
}

std::vector<std::uint8_t> unhex(const std::string& text) {
  std::vector<std::uint8_t> out;
  for (std::size_t i = 0; i + 1 < text.size(); i += 2) {
    out.push_back(static_cast<std::uint8_t>(std::stoi(text.substr(i, 2), nullptr, 16)));
  }
  return out;
}

domain::Session makeSession(std::uint32_t id, domain::Agent agent, domain::SessionState state,
                            std::uint8_t subagents, long seconds, const char* name,
                            const char* detail) {
  domain::Session s;
  s.id = domain::SessionId{id};
  s.agent = agent;
  s.state = state;
  s.subagents = subagents;
  s.inState = Millis{seconds * 1000};
  s.name = name;
  s.detail = detail;
  return s;
}

void encodeCase(const char* name, const protocol::Frame& frame, std::size_t capacity) {
  std::vector<std::uint8_t> out(capacity);
  const std::size_t n = protocol::encodeFrame(frame, out);
  std::printf("encode %s %zu %s\n", name, capacity, hex(std::span(out.data(), n)).c_str());
}

void decodeCase(const char* name, const std::vector<std::uint8_t>& bytes) {
  const auto frame = protocol::decodeFrame(bytes);
  if (!frame) {
    std::printf("decode %s %s reject\n", name, hex(bytes).c_str());
    return;
  }
  std::vector<std::uint8_t> out(protocol::kMaxFrameBytes);
  const std::size_t n = protocol::encodeFrame(*frame, out);
  std::printf("decode %s %s %s\n", name, hex(bytes).c_str(), hex(std::span(out.data(), n)).c_str());
}

void dumpWire() {
  using domain::Agent;
  using domain::SessionState;

  protocol::Frame sample;
  sample.secondsOfDay = 13 * 3600 + 7 * 60;
  sample.nightStart = 22;
  sample.nightEnd = 7;
  sample.settings.swapButtons = true;
  sample.settings.dayBrightness = 55;
  sample.snapshot.sessions.push_back(makeSession(0xDEADBEEF, Agent::Codex, SessionState::NeedsYou,
                                                 3, 42, "Миграция базы", "perm"));
  sample.snapshot.sessions.push_back(
      makeSession(7, Agent::Claude, SessionState::Working, 0, 0, "desksessions-d9", ""));
  encodeCase("sample", sample, protocol::kMaxFrameBytes);
  encodeCase("sample-short-buffer", sample, 20);

  protocol::Frame empty;
  encodeCase("empty", empty, protocol::kMaxFrameBytes);

  protocol::Frame flags;
  flags.settings.flipScreen = true;
  flags.settings.swapButtons = true;
  flags.settings.dayBrightness = 100;
  flags.settings.nightBrightness = 0;
  flags.secondsOfDay = 86'399;
  flags.nightStart = 0;
  flags.nightEnd = 23;
  encodeCase("flags", flags, protocol::kMaxFrameBytes);

  protocol::Frame truncated;
  domain::Session cut;
  cut.name.assign(47, 'a');
  cut.name.append("ж");  // etl keeps 48 bytes: 47 + half a letter
  cut.detail = "Подтверждение";  // 26 bytes into 12: etl keeps 12, cut on a boundary
  truncated.snapshot.sessions.push_back(cut);
  encodeCase("utf8-truncation", truncated, protocol::kMaxFrameBytes);

  protocol::Frame full;
  for (std::uint32_t i = 0; i < domain::kMaxSessions; ++i) {
    full.snapshot.sessions.push_back(makeSession(
        1000 + i, i % 2 ? Agent::Codex : Agent::Claude, static_cast<SessionState>(i % 3),
        static_cast<std::uint8_t>(i), 3600L * i + 59,
        "A very long session name that certainly overflows its budget", "WebFetch12345"));
  }
  encodeCase("full", full, protocol::kMaxFrameBytes);
  encodeCase("full-one-short", full, protocol::kMaxFrameBytes - 1);

  std::vector<std::uint8_t> sampleBytes(protocol::kMaxFrameBytes);
  sampleBytes.resize(protocol::encodeFrame(sample, sampleBytes));

  decodeCase("golden",
             unhex("0102f0b000001708500c02040302010000002a0000000c73657276617269756d2d3035047065"
                   "726dd0c0b0a0010203100e000019d09cd0b8d0b3d180d0b0d186d0b8d18f20d0b1d0b0d0b7d18b"
                   "00"));
  decodeCase("sample", sampleBytes);
  {
    auto b = sampleBytes;
    b.pop_back();
    decodeCase("truncated", b);
  }
  {
    auto b = sampleBytes;
    b.push_back(0);
    decodeCase("trailing", b);
  }
  {
    auto b = sampleBytes;
    b[0] = 2;
    decodeCase("version", b);
  }
  {
    auto b = sampleBytes;
    b[protocol::kFrameHeaderBytes + 5] = 9;
    decodeCase("state", b);
  }
  {
    auto b = sampleBytes;
    b[protocol::kFrameHeaderBytes + 4] = 2;
    decodeCase("agent", b);
  }
  for (std::size_t field = 6; field <= 9; ++field) {
    auto b = sampleBytes;
    b[field] = field < 8 ? 24 : 101;
    decodeCase(("header-range-" + std::to_string(field)).c_str(), b);
  }
  {
    auto b = sampleBytes;
    b[10] = 17;
    decodeCase("count", b);
  }
  {
    auto b = sampleBytes;
    b[1] = 0xFF;  // unknown flag bits are ignored
    decodeCase("flags-all", b);
  }
  {
    auto b = sampleBytes;
    b[protocol::kFrameHeaderBytes + 11] = 49;  // name length over the limit
    decodeCase("name-length", b);
  }
  decodeCase("nothing", {});
  decodeCase("full", [&] {
    std::vector<std::uint8_t> b(protocol::kMaxFrameBytes);
    b.resize(protocol::encodeFrame(full, b));
    return b;
  }());
  {
    // Invalid UTF-8 is carried through as bytes.
    auto b = unhex("0100000000001708640a01" "01000000" "000000" "05000000" "03ff80c1" "00");
    decodeCase("raw-bytes", b);
  }

  for (std::uint32_t id : {0u, 1u, 0x01020304u, 0xFFFFFFFFu}) {
    for (auto kind : {domain::Command::Kind::Dismiss, domain::Command::Kind::Jump}) {
      const auto bytes = protocol::encodeCommand({.kind = kind, .session = domain::SessionId{id}});
      std::printf("command %s\n", hex(bytes).c_str());
    }
  }
  for (const char* text : {"0004030201", "0104030201", "0204030201", "01040302", "010403020100", ""}) {
    const auto cmd = protocol::decodeCommand(unhex(text));
    if (cmd) {
      std::printf("command-decode %s %d %" PRIu32 "\n", text[0] ? text : "-",
                  static_cast<int>(cmd->kind), cmd->session.value);
    } else {
      std::printf("command-decode %s reject\n", text[0] ? text : "-");
    }
  }

  // Reassembly: a stream of chunks and what each push returns.
  const auto chunk = [](std::uint8_t seq, std::uint8_t index, std::uint8_t count,
                        std::vector<std::uint8_t> payload) {
    std::vector<std::uint8_t> c{seq, index, count};
    c.insert(c.end(), payload.begin(), payload.end());
    return c;
  };
  const std::vector<std::vector<std::uint8_t>> stream = {
      chunk(1, 0, 1, {1, 2, 3}),
      chunk(2, 0, 3, {4, 5}),
      chunk(2, 1, 3, {6}),
      chunk(3, 0, 2, {7}),  // interrupts frame 2
      chunk(3, 1, 2, {8, 9}),
      chunk(4, 1, 2, {10}),  // starts mid-frame: dropped
      chunk(4, 0, 2, {11}),
      chunk(4, 0, 2, {12}),  // restarts
      chunk(4, 1, 2, {13}),
      {5, 0},                // too short
      chunk(5, 0, 0, {1}),   // no chunks
      chunk(5, 0, 9, {1}),   // too many chunks
      chunk(5, 2, 2, {1}),   // index past count
      chunk(6, 0, 2, {1}),
      chunk(6, 0, 2, {}),    // empty payload restart
      chunk(6, 1, 2, {2}),
      chunk(7, 0, 1, {}),
  };
  protocol::Reassembler reassembler;
  for (const auto& c : stream) {
    const auto frame = reassembler.push(c);
    std::printf("chunk %s %s\n", hex(c).c_str(),
                frame ? ("frame " + hex(*frame)).c_str() : "none");
  }
  // Overflow: more payload than a frame can hold.
  protocol::Reassembler big;
  std::vector<std::uint8_t> payload(250, 0xAB);
  for (std::uint8_t i = 0; i < 8; ++i) {
    const auto frame = big.push(chunk(9, i, 8, payload));
    std::printf("big %u %s\n", i, frame ? std::to_string(frame->size()).c_str() : "none");
  }
}

// ---------------------------------------------------------------------------------------------
// curves

void row(const char* name, long i, float value) { std::printf("%s %ld %.9g\n", name, i, value); }

void dumpCurves() {
  using anim::Curve;
  const std::pair<const char*, Curve> curves[] = {
      {"linear", Curve::Linear},       {"ease-in", Curve::EaseIn},
      {"ease-out", Curve::EaseOut},    {"ease-in-out", Curve::EaseInOut},
      {"ca-default", Curve::CaDefault}, {"ca-ease-in", Curve::CaEaseIn},
      {"ca-ease-out", Curve::CaEaseOut}, {"ca-ease-in-out", Curve::CaEaseInOut},
  };
  for (const auto& [name, curve] : curves) {
    for (int i = -10; i <= 1010; ++i) {
      row(name, i, anim::ease(curve, static_cast<float>(i) / 1000.0f));
    }
  }
  const anim::CubicBezier backOut{0.34f, 1.56f, 0.64f, 1.0f};
  for (int i = 0; i <= 1000; ++i) {
    row("back-out", i, backOut(static_cast<float>(i) / 1000.0f));
  }

  struct Move {
    const char* name;
    const anim::MoookShape* shape;
    float from;
    float to;
  };
  const Move moves[] = {
      {"moook-30", &anim::kMoook, 0, 30},         {"moook-up", &anim::kMoook, 30, 0},
      {"moook-15", &anim::kMoook, 0, 15},         {"moook-exit", &anim::kMoook, 0, -320},
      {"moook-soft", &anim::kMoookSoft, -14, 0},  {"peek", &anim::kPeek, -5, 0},
      {"settle", &anim::kMoookSettle, 12.5f, 60}, {"moook-digit", &anim::kMoook, -24, 0},
      {"moook-none", &anim::kMoook, 5, 5},
  };
  for (const Move& m : moves) {
    for (int ms = -5; ms <= 340; ++ms) {
      row(m.name, ms, anim::moook(*m.shape, m.from, m.to, Millis{ms}));
    }
  }

  struct SpringCase {
    const char* name;
    anim::SpringParams params;
    float value;
    float velocity;
    float target;
  };
  const SpringCase springs[] = {
      {"spring-smooth", anim::kSmooth, 0, 0, 100},
      {"spring-snappy", anim::kSnappy, 0, 0, 100},
      {"spring-bouncy", anim::kBouncy, 0, 0, 100},
      {"spring-wiggle", anim::springFrom(0.18f, 0.55f), 0, 260, 0},
      {"spring-kick", anim::kBouncy, 1, 1.2f, 1},
  };
  for (const auto& s : springs) {
    anim::Spring spring{.value = s.value, .velocity = s.velocity, .target = s.target};
    for (int i = 0; i < 400; ++i) {
      anim::step(spring, s.params);
      row(s.name, i, spring.value);
    }
  }
  row("params-smooth-k", 0, anim::kSmooth.stiffness);
  row("params-smooth-d", 0, anim::kSmooth.damping);
  row("params-snappy-d", 0, anim::kSnappy.damping);
  row("params-bouncy-d", 0, anim::kBouncy.damping);

  for (int ms = -2000; ms <= 4000; ++ms) {
    row("breathe", ms, anim::breathe(Millis{ms}));
  }
  for (float overflow : {50.0f, 13.5f, 120.0f}) {
    const std::string name = "marquee-" + std::to_string(static_cast<int>(overflow * 10));
    for (int ms = 0; ms <= 9000; ms += 3) {
      row(name.c_str(), ms, anim::marqueeOffset(overflow, Millis{ms}));
    }
  }
  for (int i = -200; i <= 200; ++i) {
    row("rubber-band", i, anim::rubberBand(static_cast<float>(i) * 1.5f, 100.0f));
  }
  for (int i = -10; i <= 1200; ++i) {
    row("progress", i, progress(Millis{i}, Millis{1000}));
  }

  // Channels: scripted retargets, sampled every frame.
  struct Script {
    const char* name;
    void (*run)(anim::Channel&, Instant);
  };
  using anim::Motion;
  const Script scripts[] = {
      {"channel-tween-reverse",
       [](anim::Channel& c, Instant t) {
         c.animateTo(100, Motion::tween(Curve::EaseInOut, Millis{250}), t);
       }},
      {"channel-spring", [](anim::Channel& c,
                            Instant t) { c.animateTo(100, Motion::springy(anim::kSnappy), t); }},
      {"channel-moook", [](anim::Channel& c,
                           Instant t) { c.animateTo(30, Motion::moook(anim::kMoook), t); }},
      {"channel-kick", [](anim::Channel& c, Instant t) { c.kick(1.2f, t); }},
      {"channel-then",
       [](anim::Channel& c, Instant t) {
         c.animateTo(-5, Motion::tween(Curve::EaseOut, Millis{48}), t);
         c.then(0, Motion::tween(Curve::EaseOut, Millis{144}));
       }},
  };
  for (const Script& s : scripts) {
    anim::Channel channel{0.0f};
    Instant now{Millis{1'000}};
    s.run(channel, now);
    for (int frame = 0; frame < 120; ++frame) {
      now = now + Millis{frame % 3 == 2 ? 17 : 16};
      if (frame == 7) {
        // Interrupt: back to zero with the same kind of motion, or a spring.
        channel.animateTo(frame % 2 ? 0.0f : 12.0f, Motion::springy(anim::kSmooth), now);
      }
      if (frame == 30) {
        channel.animateTo(60, Motion::moook(anim::kMoook), now);
      }
      if (frame == 33) {
        channel.animateTo(90, Motion::moook(anim::kMoook), now);
      }
      if (frame == 60) {
        channel.animateTo(10, Motion::tween(Curve::CaEaseInOut, Millis{400}), now);
      }
      if (frame == 66) {
        channel.animateTo(-40, Motion::tween(Curve::CaEaseInOut, Millis{400}), now);
      }
      channel.update(now);
      row(s.name, frame, channel.value());
      row((std::string(s.name) + "-velocity").c_str(), frame, channel.velocity());
    }
  }
}

// ---------------------------------------------------------------------------------------------
// trace: the simulator's loop without the window

constexpr int kFps = 60;

class SimClock {
 public:
  explicit SimClock(double speed) : speed_(speed) {}
  Instant tick() {
    elapsed_ += 1000.0 / kFps * speed_;
    return now();
  }
  [[nodiscard]] Instant now() const {
    return Instant{Millis{static_cast<Millis::rep>(kStart + elapsed_)}};
  }
  [[nodiscard]] Millis sinceLaunch() const { return Millis{static_cast<Millis::rep>(elapsed_)}; }

 private:
  static constexpr double kStart = 100'000.0;
  double elapsed_ = 0.0;
  double speed_;
};

class SimLink {
 public:
  void toggle(app::DeskApp& deskApp, Instant now) {
    connected_ = !connected_;
    deskApp.linkChanged(
        {.kind = connected_ ? app::LinkEvent::Kind::Connected : app::LinkEvent::Kind::Disconnected},
        now);
  }
  void sleep(app::DeskApp& deskApp, Instant now) {
    connected_ = false;
    deskApp.linkChanged({.kind = app::LinkEvent::Kind::Disconnected}, now - Millis{300'000});
  }
  void pair(app::DeskApp& deskApp, Instant now) {
    connected_ = false;
    pairingFrom_ = now;
    deskApp.linkChanged({.kind = app::LinkEvent::Kind::Unpaired}, now);
  }
  void advance(app::DeskApp& deskApp, Instant now) {
    if (!pairingFrom_) {
      return;
    }
    const Millis t = now - *pairingFrom_;
    if (step_ == 0 && t >= Millis{2'500}) {
      deskApp.linkChanged({.kind = app::LinkEvent::Kind::Passkey, .passkey = 482913}, now);
      step_ = 1;
    } else if (step_ == 1 && t >= Millis{6'500}) {
      deskApp.linkChanged({.kind = app::LinkEvent::Kind::Connected}, now);
      connected_ = true;
      pairingFrom_.reset();
      step_ = 0;
    }
  }

 private:
  bool connected_ = true;
  std::optional<Instant> pairingFrom_;
  int step_ = 0;
};

class RecordingBacklight final : public app::Backlight {
 public:
  void set(float level) override { level_ = level; }
  float level_ = 1.0f;
};

bool cueHolds(const std::vector<sim::Cue>& cues, sim::Cue::Action action, Millis t) {
  for (const sim::Cue& cue : cues) {
    if (cue.action == action && t >= cue.at && t < cue.at + cue.holdFor) {
      return true;
    }
  }
  return false;
}

void dumpState(const app::DeskApp& app, Millis t, Instant now, float backlight) {
  const auto& scene = app.scene();
  const auto& stage = app.stage();
  std::printf("t %lld now %lld phase %d cursor %zu rows %zu bl %.9g", static_cast<long long>(t.count()),
              static_cast<long long>(now.sinceBoot.count()), static_cast<int>(app.phase()),
              app.board().cursor(), app.board().rows().size(), backlight);
  std::printf(" hl %.9g hlo %.9g scroll %.9g nudge %.9g fill %.9g", scene.highlightY(),
              scene.highlightOpacity(), scene.scroll(), scene.nudgeOffset(), scene.holdFill());
  std::printf(" counts %.9g %.9g %.9g", scene.count(domain::SessionState::NeedsYou),
              scene.count(domain::SessionState::Done), scene.count(domain::SessionState::Working));
  std::printf(" stage %.9g %.9g %.9g %.9g %.9g", stage.quiet(), stage.offline(), stage.asleep(),
              stage.pairing(), stage.wake());
  std::printf(" sel %u", scene.selectedId() ? scene.selectedId()->value : 0u);
  for (const auto& slot : scene.slots()) {
    std::printf(" | %u %d %d %d %.9g %.9g %.9g %.9g %.9g %.9g %.9g", slot.session.id.value,
                slot.exiting ? 1 : 0, static_cast<int>(slot.session.state),
                static_cast<int>(slot.previousState), slot.y.value(), slot.x.value(),
                slot.opacity.value(), slot.lift.value(), slot.stateBlend.value(),
                slot.scale.value(), slot.glyphScale.value());
  }
  std::printf("\n");
}

int dumpTrace(int argc, char** argv) {
  const auto parsed = sim::parse(argc, argv);
  if (!parsed || !parsed->stopAfter) {
    std::fprintf(stderr, "trace needs --seconds or --at\n");
    return 2;
  }
  const sim::Options& options = *parsed;
  SimClock clock(options.speed);
  demo::Scenario scenario(clock.now());
  if (!options.demo) {
    scenario.toggleAuto();
  }
  RecordingBacklight backlight;
  app::DeskApp deskApp(scenario, backlight);
  deskApp.setTime(options.secondsOfDay.value_or(12 * 3600), clock.now());
  deskApp.linkChanged({.kind = app::LinkEvent::Kind::Connected}, clock.now());
  SimLink link;
  std::size_t nextCue = 0;

  for (;;) {
    const Instant now = clock.tick();
    const Millis t = clock.sinceLaunch();
    if (scenario.takeForgetRequest()) {
      link.pair(deskApp, now);
    }
    for (; nextCue < options.cues.size() && options.cues[nextCue].at <= t; ++nextCue) {
      switch (options.cues[nextCue].action) {
        case sim::Cue::Action::Add:
          scenario.addSession(now);
          break;
        case sim::Cue::Action::Remove:
          scenario.removeLast();
          break;
        case sim::Cue::Action::Escalate:
          scenario.escalateFirstWorking(now);
          break;
        case sim::Cue::Action::Pause:
          scenario.toggleAuto();
          break;
        case sim::Cue::Action::Link:
          link.toggle(deskApp, now);
          break;
        case sim::Cue::Action::Sleep:
          link.sleep(deskApp, now);
          break;
        case sim::Cue::Action::Pair:
          link.pair(deskApp, now);
          break;
        case sim::Cue::Action::Empty:
          scenario.toggleEmpty();
          break;
        default:
          break;
      }
    }
    link.advance(deskApp, now);
    if (scenario.advance(now)) {
      deskApp.receive(scenario.snapshot(now), now);
    }
    const bool bootDown = cueHolds(options.cues, sim::Cue::Action::Up, t);
    const bool auxDown = cueHolds(options.cues, sim::Cue::Action::Down, t);
    deskApp.tick({.bootDown = bootDown, .auxDown = auxDown}, now);
    if (scenario.advance(now)) {
      deskApp.receive(scenario.snapshot(now), now);
    }
    dumpState(deskApp, t, now, backlight.level_);
    if (t >= *options.stopAfter) {
      return 0;
    }
  }
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 2) {
    std::fprintf(stderr, "usage: desk-dump wire|curves|trace ...\n");
    return 2;
  }
  const std::string_view what = argv[1];
  if (what == "wire") {
    dumpWire();
    return 0;
  }
  if (what == "curves") {
    dumpCurves();
    return 0;
  }
  if (what == "trace") {
    return dumpTrace(argc - 1, argv + 1);
  }
  std::fprintf(stderr, "unknown: %s\n", argv[1]);
  return 2;
}
