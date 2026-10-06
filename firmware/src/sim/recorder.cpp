#include "recorder.hpp"

namespace desk::sim {
namespace {

// 3× nearest-neighbour so pixels stay crisp, and a per-clip palette so colours stay true.
constexpr const char* kScale = "scale=iw*3:ih*3:flags=neighbor";

}  // namespace

void Recorder::read(ui::Canvas& canvas, std::vector<std::uint8_t>& rgb) {
  const auto w = static_cast<std::size_t>(canvas.width());
  const auto h = static_cast<std::size_t>(canvas.height());
  rgb.resize(w * h * 3);
  canvas.readRectRGB(0, 0, canvas.width(), canvas.height(), rgb.data());
}

void Recorder::still(ui::Canvas& canvas, const std::string& path) {
  read(canvas, rgb_);
  const std::string command =
      "ffmpeg -loglevel error -y -f rawvideo -pix_fmt rgb24 -s " + std::to_string(canvas.width()) +
      "x" + std::to_string(canvas.height()) + " -i - -vf " + kScale + " '" + path + "'";
  if (std::FILE* out = popen(command.c_str(), "w")) {
    std::fwrite(rgb_.data(), 1, rgb_.size(), out);
    pclose(out);
  }
}

void Recorder::start(const std::string& path, int fps) {
  stop();
  const std::string input = "ffmpeg -loglevel error -y -f rawvideo -pix_fmt rgb24 -s 320x170 -r " +
                            std::to_string(fps) + " -i - -vf '" + kScale;
  const bool mp4 = path.size() > 4 && path.substr(path.size() - 4) == ".mp4";
  const std::string command =
      mp4 ? input + "' -c:v libx264 -preset slow -crf 16 -pix_fmt yuv420p -movflags +faststart '" +
                path + "'"
          : input +
                ",split[a][b];[a]palettegen=stats_mode=full[p];[b][p]paletteuse=dither=none' '" +
                path + "'";
  pipe_ = popen(command.c_str(), "w");
}

void Recorder::frame(ui::Canvas& canvas) {
  if (pipe_ == nullptr) {
    return;
  }
  read(canvas, rgb_);
  std::fwrite(rgb_.data(), 1, rgb_.size(), pipe_);
}

void Recorder::stop() {
  if (pipe_ != nullptr) {
    pclose(pipe_);
    pipe_ = nullptr;
  }
}

}  // namespace desk::sim
