// Simulator: the firmware's app and renderer in an SDL window.
//
//   ↑ / ↓   BOOT / GPIO21 (hold for a long press, both for the re-pair chord)
//   A       add a session        X   remove the newest session
//   N       a working session needs you
//   P       pause / resume the scripted demo
//   S       slow motion (¼ speed)
//   L       drop / restore the link to the Mac (offline after 10 s)
//   Z       drop the link as if 5 minutes ago (straight to asleep)
//   U       forget pairing and walk through pairing again
//   E       empty the list / bring it back
//   R       start / stop recording a GIF into recordings/
//
// Run with --help-style options from options.hpp for scripted recordings and stills.

#define LGFX_AUTODETECT
#include <LGFX_AUTODETECT.hpp>
#include <LovyanGFX.hpp>
#include <chrono>
#include <ctime>
#include <filesystem>
#include <optional>
#include <string>

#include "desk/app/desk_app.hpp"
#include "desk/ui/canvas.hpp"
#include "desk/ui/renderer.hpp"
#include "options.hpp"
#include "recorder.hpp"
#include "scenario.hpp"

namespace {

using namespace desk;

enum Pin : std::uint8_t {
  kBoot = 0,
  kAux = 21,
  kAdd = 100,
  kRemove,
  kEscalate,
  kPause,
  kSlowMotion,
  kRecord,
  kLink,
  kSleep,
  kPair,
  kEmpty,
};

constexpr int kFps = 60;

/// Simulated monotonic time. Live, it follows the wall clock (optionally slowed). When capturing,
/// it advances exactly one frame per frame so recordings are smooth and repeatable.
class SimClock {
 public:
  SimClock(double speed, bool fixedStep) : speed_(speed), fixedStep_(fixedStep) {}

  Instant tick() {
    if (fixedStep_) {
      elapsed_ += 1000.0 / kFps * speed_;
    } else {
      const auto real = std::chrono::steady_clock::now();
      elapsed_ += std::chrono::duration<double, std::milli>(real - last_).count() * speed_;
      last_ = real;
    }
    return now();
  }
  [[nodiscard]] Instant now() const {
    return Instant{Millis{static_cast<Millis::rep>(kStart + elapsed_)}};
  }
  [[nodiscard]] Millis sinceLaunch() const { return Millis{static_cast<Millis::rep>(elapsed_)}; }
  void toggleSlowMotion() { speed_ = speed_ < 1.0 ? 1.0 : 0.25; }

 private:
  static constexpr double kStart = 100'000.0;  // "seconds ago" seeds stay positive
  std::chrono::steady_clock::time_point last_ = std::chrono::steady_clock::now();
  double elapsed_ = 0.0;
  double speed_;
  bool fixedStep_;
};

/// A pretend BLE link: connected, lost, or walking through pairing on a timer.
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

std::optional<app::WallTime> wallTime() {
  const std::time_t t = std::time(nullptr);
  std::tm local{};
  localtime_r(&t, &local);
  return app::WallTime{static_cast<std::uint8_t>(local.tm_hour),
                       static_cast<std::uint8_t>(local.tm_min)};
}

/// Edge detector for the emulated GPIOs (active low, like real buttons).
class Key {
 public:
  explicit Key(std::uint8_t pin) : pin_(pin) {}
  [[nodiscard]] bool down() const { return !lgfx::gpio_in(pin_); }
  bool pressed() {
    const bool now = down();
    const bool edge = now && !was_;
    was_ = now;
    return edge;
  }

 private:
  std::uint8_t pin_;
  bool was_ = false;
};

/// Whether a scripted cue is holding `action`'s button at `t`.
bool cueHolds(const std::vector<sim::Cue>& cues, sim::Cue::Action action, Millis t) {
  for (const sim::Cue& cue : cues) {
    if (cue.action == action && t >= cue.at && t < cue.at + cue.holdFor) {
      return true;
    }
  }
  return false;
}

LGFX lcd(320, 170, 3);
sim::Options options;

int run(bool* running) {
  lcd.init();
  static_cast<lgfx::Panel_sdl*>(lcd.getPanel())->setWindowTitle("desksessions");

  ui::Canvas canvas(&lcd);
  canvas.setColorDepth(16);
  canvas.createSprite(320, 170);

  const bool capturing = options.gif.has_value() || !options.shots.empty();
  SimClock clock(options.speed, capturing);
  sim::Scenario scenario(clock.now());
  if (!options.demo) {
    scenario.toggleAuto();
  }
  app::DeskApp deskApp(scenario);
  deskApp.linkChanged({.kind = app::LinkEvent::Kind::Connected}, clock.now());
  ui::Renderer renderer(canvas);
  sim::Recorder recorder;
  if (options.gif) {
    recorder.start(*options.gif, kFps);
  }

  Key boot(kBoot), aux(kAux), add(kAdd), remove(kRemove), escalate(kEscalate), pause(kPause),
      slow(kSlowMotion), record(kRecord), linkKey(kLink), sleepKey(kSleep), pairKey(kPair),
      emptyKey(kEmpty);
  SimLink link;
  std::size_t nextCue = 0;
  std::size_t nextShot = 0;
  int clip = 0;

  constexpr auto kFrame = std::chrono::microseconds(1'000'000 / kFps);
  auto nextFrame = std::chrono::steady_clock::now();
  while (*running) {
    const Instant now = clock.tick();
    const Millis t = clock.sinceLaunch();

    if (add.pressed()) scenario.addSession(now);
    if (remove.pressed()) scenario.removeLast();
    if (escalate.pressed()) scenario.escalateFirstWorking(now);
    if (pause.pressed()) scenario.toggleAuto();
    if (slow.pressed()) clock.toggleSlowMotion();
    if (linkKey.pressed()) link.toggle(deskApp, now);
    if (sleepKey.pressed()) link.sleep(deskApp, now);
    if (pairKey.pressed() || scenario.takeForgetRequest()) link.pair(deskApp, now);
    if (emptyKey.pressed()) scenario.toggleEmpty();
    if (record.pressed()) {
      if (recorder.recording()) {
        recorder.stop();
        std::printf("[sim] recording saved\n");
      } else {
        std::filesystem::create_directories("recordings");
        const std::string path = "recordings/clip-" + std::to_string(++clip) + ".gif";
        recorder.start(path, kFps);
        std::printf("[sim] recording %s\n", path.c_str());
      }
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
    const bool bootDown = boot.down() || cueHolds(options.cues, sim::Cue::Action::Up, t);
    const bool auxDown = aux.down() || cueHolds(options.cues, sim::Cue::Action::Down, t);
    deskApp.tick({.bootDown = bootDown, .auxDown = auxDown}, now);
    if (scenario.advance(now)) {  // a dismissal this tick
      deskApp.receive(scenario.snapshot(now), now);
    }

    renderer.draw(deskApp, now, wallTime());
    canvas.pushSprite(0, 0);
    recorder.frame(canvas);

    for (; nextShot < options.shots.size() && options.shots[nextShot] <= t; ++nextShot) {
      recorder.still(canvas, options.shotsDir + "/shot-" +
                                 std::to_string(options.shots[nextShot].count()) + ".png");
    }
    if (options.stopAfter && t >= *options.stopAfter) {
      recorder.stop();
      std::exit(0);
    }

    if (!capturing) {
      nextFrame += kFrame;
      while (std::chrono::steady_clock::now() < nextFrame) {
        lgfx::delay(1);
      }
    }
  }
  return 0;
}

}  // namespace

int main(int argc, char** argv) {
  const auto parsed = sim::parse(argc, argv);
  if (!parsed) {
    return 2;
  }
  options = *parsed;
  if (!options.shotsDir.empty()) {
    std::filesystem::create_directories(options.shotsDir);
  }

  // Registered before Panel_sdl's defaults so the arrows reach the firmware's button pins.
  lgfx::Panel_sdl::addKeyCodeMapping(SDLK_UP, kBoot);
  lgfx::Panel_sdl::addKeyCodeMapping(SDLK_DOWN, kAux);
  lgfx::Panel_sdl::addKeyCodeMapping(SDLK_a, kAdd);
  lgfx::Panel_sdl::addKeyCodeMapping(SDLK_x, kRemove);
  lgfx::Panel_sdl::addKeyCodeMapping(SDLK_n, kEscalate);
  lgfx::Panel_sdl::addKeyCodeMapping(SDLK_p, kPause);
  lgfx::Panel_sdl::addKeyCodeMapping(SDLK_s, kSlowMotion);
  lgfx::Panel_sdl::addKeyCodeMapping(SDLK_r, kRecord);
  lgfx::Panel_sdl::addKeyCodeMapping(SDLK_l, kLink);
  lgfx::Panel_sdl::addKeyCodeMapping(SDLK_z, kSleep);
  lgfx::Panel_sdl::addKeyCodeMapping(SDLK_u, kPair);
  lgfx::Panel_sdl::addKeyCodeMapping(SDLK_e, kEmpty);
  return lgfx::Panel_sdl::main(run);
}
