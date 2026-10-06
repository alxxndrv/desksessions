#pragma once

#include <optional>
#include <string>
#include <vector>

#include "desk/util/time.hpp"

namespace desk::sim {

/// A scripted input: press or hold a button, or poke the pretend Mac, at a time since launch.
struct Cue {
  enum class Action { Up, Down, Add, Remove, Escalate, Pause };
  Millis at{0};
  Action action = Action::Down;
  Millis holdFor{90};
};

struct Options {
  bool demo = true;                 // scripted Mac activity
  double speed = 1.0;               // < 1 is slow motion
  std::optional<std::string> gif;   // record to this file
  std::optional<Millis> stopAfter;  // quit after this much simulated time
  std::string shotsDir;             // where --shot stills go
  std::vector<Millis> shots;        // simulated times to take stills at
  std::vector<Cue> cues;
};

/// Parses:
///   --no-demo
///   --slowmo F                  simulated speed, e.g. 0.25
///   --gif PATH --seconds N      record N simulated seconds, then quit
///   --shots DIR --at T1,T2,...  PNG stills at simulated ms, then quit
///   --script "T:act,..."        act ∈ up, down, longup, longdown, add, remove, needs, pause
std::optional<Options> parse(int argc, char** argv);

}  // namespace desk::sim
