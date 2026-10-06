#include "desk/ui/fonts.hpp"

namespace desk::ui {

Fonts::Loaded::Loaded(const std::uint8_t* data) {
  source.set(data);
  font.loadFont(&source);
}

Fonts::Fonts()
    : name_(fontdata::kGolos16Medium),
      small_(fontdata::kGolos12Regular),
      label_(fontdata::kGolos12Medium),
      digits_(fontdata::kGolos34SemiBold) {}

}  // namespace desk::ui
