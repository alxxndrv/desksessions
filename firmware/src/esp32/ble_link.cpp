#include "ble_link.hpp"

#include <Arduino.h>
#include <esp_random.h>

#include <cstdio>

namespace desk::esp32 {
namespace {

constexpr const char* kServiceUuid = "b4d4f873-8133-48f0-9479-88481c6d52cf";
constexpr const char* kSnapshotUuid = "8c9ccb7b-95a9-411e-a3db-00b974bbb1ea";
constexpr const char* kEventsUuid = "6fbc6994-1373-4ace-9710-e9bf939d9251";

std::string deviceName() {
  const NimBLEAddress address = NimBLEDevice::getAddress();
  const std::uint8_t* raw = address.getVal();
  char name[16];
  std::snprintf(name, sizeof(name), "desk-%02X%02X", raw[1], raw[0]);
  return name;
}

void startAdvertising() {
  NimBLEAdvertising* advertising = NimBLEDevice::getAdvertising();
  advertising->stop();
  advertising->reset();
  advertising->addServiceUUID(kServiceUuid);
  advertising->setName(deviceName());
  advertising->enableScanResponse(true);
  advertising->start();
}

}  // namespace

void BleLink::begin() {
  NimBLEDevice::init("desk");
  NimBLEDevice::setDeviceName(deviceName());
  NimBLEDevice::setMTU(517);
  NimBLEDevice::setPower(9);

  // Bonded, MITM-protected LE Secure Connections; the clock shows a passkey the Mac types in.
  NimBLEDevice::setSecurityAuth(true, true, true);
  NimBLEDevice::setSecurityIOCap(BLE_HS_IO_DISPLAY_ONLY);

  server_ = NimBLEDevice::createServer();
  server_->setCallbacks(this, false);
  server_->advertiseOnDisconnect(true);

  NimBLEService* service = server_->createService(kServiceUuid);
  NimBLECharacteristic* snapshot = service->createCharacteristic(
      kSnapshotUuid,
      NIMBLE_PROPERTY::WRITE | NIMBLE_PROPERTY::WRITE_ENC | NIMBLE_PROPERTY::WRITE_AUTHEN);
  snapshot->setCallbacks(this);
  events_ = service->createCharacteristic(
      kEventsUuid, NIMBLE_PROPERTY::NOTIFY | NIMBLE_PROPERTY::READ | NIMBLE_PROPERTY::READ_ENC |
                       NIMBLE_PROPERTY::READ_AUTHEN);
  service->start();

  startAdvertising();
  postIdle();
}

void BleLink::send(const domain::Command& command) {
  if (events_ == nullptr || server_->getConnectedCount() == 0) {
    return;
  }
  const auto bytes = protocol::encodeCommand(command);
  events_->setValue(bytes.data(), bytes.size());
  events_->notify();
}

void BleLink::forgetPairing() {
  NimBLEDevice::deleteAllBonds();
  for (std::uint16_t handle : server_->getPeerDevices()) {
    server_->disconnect(handle);
  }
  post(app::LinkEvent{.kind = app::LinkEvent::Kind::Unpaired});
}

void BleLink::onConnect(NimBLEServer* /*server*/, NimBLEConnInfo& info) {
  // Ask for encryption straight away: pairing if new, re-encryption if bonded.
  NimBLEDevice::startSecurity(info.getConnHandle());
}

void BleLink::onDisconnect(NimBLEServer* /*server*/, NimBLEConnInfo& /*info*/, int /*reason*/) {
  postIdle();
}

uint32_t BleLink::onPassKeyDisplay() {
  const std::uint32_t passkey = esp_random() % 1'000'000;
  passkey_ = passkey;
  post(app::LinkEvent{.kind = app::LinkEvent::Kind::Passkey, .passkey = passkey});
  return passkey;
}

void BleLink::onAuthenticationComplete(NimBLEConnInfo& info) {
  if (!info.isEncrypted() || !info.isBonded()) {
    server_->disconnect(info.getConnHandle());
    return;
  }
  // One Mac per clock: a second bond means someone else paired. Undo it.
  if (NimBLEDevice::getNumBonds() > 1 &&
      !(NimBLEDevice::getBondedAddress(0) == info.getIdAddress())) {
    NimBLEDevice::deleteBond(info.getIdAddress());
    server_->disconnect(info.getConnHandle());
    return;
  }
  post(app::LinkEvent{.kind = app::LinkEvent::Kind::Connected});
}

void BleLink::onWrite(NimBLECharacteristic* characteristic, NimBLEConnInfo& /*info*/) {
  const NimBLEAttValue& value = characteristic->getValue();
  Chunk chunk;
  chunk.size = static_cast<std::uint16_t>(std::min<std::size_t>(value.size(), chunk.bytes.size()));
  std::memcpy(chunk.bytes.data(), value.data(), chunk.size);
  post(chunk);
}

void BleLink::post(const LinkMessage& message) {
  // If the UI task has fallen this far behind, dropping is better than blocking the BLE host.
  (void)inbox_.push(message);
}

void BleLink::postIdle() {
  post(app::LinkEvent{.kind = NimBLEDevice::getNumBonds() > 0 ? app::LinkEvent::Kind::Disconnected
                                                              : app::LinkEvent::Kind::Unpaired});
}

}  // namespace desk::esp32
