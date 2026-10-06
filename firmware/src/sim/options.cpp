#include "options.hpp"

#include <cstdio>
#include <cstdlib>
#include <sstream>
#include <string_view>

namespace desk::sim {
namespace {

std::vector<std::string> split(const std::string& text, char sep) {
  std::vector<std::string> parts;
  std::stringstream stream(text);
  std::string part;
  while (std::getline(stream, part, sep)) {
    if (!part.empty()) {
      parts.push_back(part);
    }
  }
  return parts;
}

std::optional<Cue> parseCue(const std::string& text) {
  const auto colon = text.find(':');
  if (colon == std::string::npos) {
    return std::nullopt;
  }
  Cue cue;
  cue.at = Millis{std::atoll(text.substr(0, colon).c_str())};
  const std::string_view act = std::string_view(text).substr(colon + 1);
  constexpr Millis kLong{800};
  if (act == "up") {
    cue.action = Cue::Action::Up;
  } else if (act == "down") {
    cue.action = Cue::Action::Down;
  } else if (act == "longup") {
    cue.action = Cue::Action::Up;
    cue.holdFor = kLong;
  } else if (act == "longdown") {
    cue.action = Cue::Action::Down;
    cue.holdFor = kLong;
  } else if (act == "add") {
    cue.action = Cue::Action::Add;
  } else if (act == "remove") {
    cue.action = Cue::Action::Remove;
  } else if (act == "needs") {
    cue.action = Cue::Action::Escalate;
  } else if (act == "pause") {
    cue.action = Cue::Action::Pause;
  } else if (act == "link") {
    cue.action = Cue::Action::Link;
  } else if (act == "sleep") {
    cue.action = Cue::Action::Sleep;
  } else if (act == "pair") {
    cue.action = Cue::Action::Pair;
  } else if (act == "empty") {
    cue.action = Cue::Action::Empty;
  } else {
    return std::nullopt;
  }
  return cue;
}

}  // namespace

std::optional<Options> parse(int argc, char** argv) {
  Options options;
  for (int i = 1; i < argc; ++i) {
    const std::string_view arg = argv[i];
    const bool hasValue = i + 1 < argc;
    if (arg == "--no-demo") {
      options.demo = false;
    } else if (arg == "--slowmo" && hasValue) {
      options.speed = std::atof(argv[++i]);
    } else if (arg == "--gif" && hasValue) {
      options.gif = argv[++i];
    } else if (arg == "--seconds" && hasValue) {
      options.stopAfter = Millis{static_cast<Millis::rep>(std::atof(argv[++i]) * 1000.0)};
    } else if (arg == "--shots" && hasValue) {
      options.shotsDir = argv[++i];
    } else if (arg == "--at" && hasValue) {
      for (const std::string& t : split(argv[++i], ',')) {
        options.shots.push_back(Millis{std::atoll(t.c_str())});
      }
    } else if (arg == "--time" && hasValue) {
      int hour = 0;
      int minute = 0;
      if (std::sscanf(argv[++i], "%d:%d", &hour, &minute) != 2) {
        std::fprintf(stderr, "bad --time, want HH:MM\n");
        return std::nullopt;
      }
      options.secondsOfDay = static_cast<std::uint32_t>(hour * 3600 + minute * 60);
    } else if (arg == "--script" && hasValue) {
      for (const std::string& item : split(argv[++i], ',')) {
        const auto cue = parseCue(item);
        if (!cue) {
          std::fprintf(stderr, "bad cue: %s\n", item.c_str());
          return std::nullopt;
        }
        options.cues.push_back(*cue);
      }
    } else {
      std::fprintf(stderr, "unknown option: %s\n", argv[i]);
      return std::nullopt;
    }
  }
  if (!options.shots.empty() && !options.stopAfter) {
    Millis last{0};
    for (Millis t : options.shots) {
      last = std::max(last, t);
    }
    options.stopAfter = last + Millis{1};
  }
  return options;
}

}  // namespace desk::sim
