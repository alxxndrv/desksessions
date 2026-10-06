#pragma once

#include <NimBLEDevice.h>
#include <etl/queue_spsc_atomic.h>

#include <array>
#include <atomic>
#include <cstdint>
#include <variant>

#include "desk/app/connection.hpp"
#include "desk/app/ports.hpp"
#include "desk/protocol/wire.hpp"

namespace desk::esp32 {

/// One snapshot chunk as written by the Mac.
struct Chunk {
  std::array<std::uint8_t, 512> bytes{};
  std::uint16_t size = 0;
};

using LinkMessage = std::variant<Chunk, app::LinkEvent>;

/// The BLE peripheral. NimBLE calls back on its own host task; everything it learns is copied
/// into a lock-free queue that the UI task drains, so no app state is ever touched from there.
class BleLink final : public app::MacLink,
                      private NimBLEServerCallbacks,
                      private NimBLECharacteristicCallbacks {
 public:
  void begin();

  /// Next message for the UI task, if any.
  bool pop(LinkMessage& out) { return inbox_.pop(out); }

  // app::MacLink, called from the UI task.
  void send(const domain::Command& command) override;
  void forgetPairing() override;

 private:
  // NimBLEServerCallbacks
  void onConnect(NimBLEServer* server, NimBLEConnInfo& info) override;
  void onDisconnect(NimBLEServer* server, NimBLEConnInfo& info, int reason) override;
  uint32_t onPassKeyDisplay() override;
  void onAuthenticationComplete(NimBLEConnInfo& info) override;

  // NimBLECharacteristicCallbacks
  void onWrite(NimBLECharacteristic* characteristic, NimBLEConnInfo& info) override;

  void post(const LinkMessage& message);
  void postIdle();

  etl::queue_spsc_atomic<LinkMessage, 12> inbox_;
  NimBLEServer* server_ = nullptr;
  NimBLECharacteristic* events_ = nullptr;
  std::atomic<std::uint32_t> passkey_{0};
};

}  // namespace desk::esp32
