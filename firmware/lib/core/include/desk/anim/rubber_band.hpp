#pragma once

namespace desk::anim {

/// UIScrollView-style resistance: how far content moves when dragged `offset` past its edge.
/// Behaves like `offset · c` for small offsets and never exceeds `dimension`.
[[nodiscard]] float rubberBand(float offset, float dimension, float c = 0.55f);

}  // namespace desk::anim
