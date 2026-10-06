#pragma once

#include <cstdio>
#include <string>
#include <vector>

#include "desk/ui/canvas.hpp"

namespace desk::sim {

/// Captures frames from the canvas: single PNG stills, or a GIF streamed through ffmpeg.
class Recorder {
 public:
  ~Recorder() { stop(); }

  void still(ui::Canvas& canvas, const std::string& path);

  void start(const std::string& gifPath, int fps);
  void frame(ui::Canvas& canvas);
  void stop();
  [[nodiscard]] bool recording() const { return pipe_ != nullptr; }

 private:
  static void read(ui::Canvas& canvas, std::vector<std::uint8_t>& rgb);

  std::FILE* pipe_ = nullptr;
  std::vector<std::uint8_t> rgb_;
};

}  // namespace desk::sim
