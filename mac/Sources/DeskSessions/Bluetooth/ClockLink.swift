import CoreBluetooth
import DeskCore
import Foundation
import Observation

/// The Mac's side of the BLE link: finds the paired clock, keeps reconnecting to it, writes
/// snapshot frames in chunks and turns the clock's notifications into commands.
@MainActor
@Observable
final class ClockLink: NSObject {
    enum Status: Equatable {
        case bluetoothOff
        case unauthorized
        case notPaired
        case searching
        case pairing
        case connected(name: String)
    }

    struct Nearby: Identifiable, Equatable {
        let id: UUID
        let name: String
        let rssi: Int
    }

    private(set) var status: Status = .searching
    private(set) var nearby: [Nearby] = []
    private(set) var isScanning = false

    @ObservationIgnored var onCommand: ((Wire.Command) -> Void)?
    @ObservationIgnored var makeFrame: (() -> Wire.Frame)?

    @ObservationIgnored private var central: CBCentralManager!
    @ObservationIgnored private var peripheral: CBPeripheral?
    @ObservationIgnored private var snapshot: CBCharacteristic?
    @ObservationIgnored private var events: CBCharacteristic?
    @ObservationIgnored private var queue: [Data] = []
    @ObservationIgnored private var writing = false
    @ObservationIgnored private var dirty = false
    @ObservationIgnored private var sequence: UInt8 = 0
    @ObservationIgnored private var debounce: Timer?
    @ObservationIgnored private var heartbeat: Timer?

    private static let pairedKey = "pairedClock"
    private let serviceUUID = CBUUID(string: Wire.serviceUUID)
    private let snapshotUUID = CBUUID(string: Wire.snapshotUUID)
    private let eventsUUID = CBUUID(string: Wire.eventsUUID)

    private var pairedID: UUID? {
        get { UserDefaults.standard.string(forKey: Self.pairedKey).flatMap(UUID.init) }
        set { UserDefaults.standard.set(newValue?.uuidString, forKey: Self.pairedKey) }
    }

    func start() {
        central = CBCentralManager(delegate: self, queue: .main)
        // A periodic frame keeps the clock's time honest and recovers from any lost write.
        heartbeat = Timer.scheduledTimer(withTimeInterval: 30, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.requestSend() }
        }
    }

    /// Something changed: send a fresh frame shortly, coalescing bursts.
    func requestSend() {
        dirty = true
        debounce?.invalidate()
        debounce = Timer.scheduledTimer(withTimeInterval: 0.08, repeats: false) { [weak self] _ in
            MainActor.assumeIsolated { self?.flush() }
        }
    }

    // MARK: Pairing

    func startPairing() {
        guard central.state == .poweredOn else { return }
        nearby = []
        isScanning = true
        central.scanForPeripherals(withServices: [serviceUUID])
    }

    func stopPairing() {
        isScanning = false
        central.stopScan()
    }

    func pair(with id: UUID) {
        stopPairing()
        disconnect()
        pairedID = id
        status = .pairing
        connectToPaired()
    }

    func forget() {
        disconnect()
        pairedID = nil
        status = .notPaired
    }

    // MARK: Connection

    private func connectToPaired() {
        guard central.state == .poweredOn, let id = pairedID,
              let known = central.retrievePeripherals(withIdentifiers: [id]).first
        else {
            status = pairedID == nil ? .notPaired : .searching
            return
        }
        peripheral = known
        known.delegate = self
        if status != .pairing { status = .searching }
        // A pending connect never times out: it completes whenever the clock is back in range.
        central.connect(known)
    }

    private func disconnect() {
        if let peripheral { central.cancelPeripheralConnection(peripheral) }
        peripheral = nil
        snapshot = nil
        events = nil
        queue = []
        writing = false
    }

    private func flush() {
        guard dirty, !writing, let peripheral, let snapshot, let frame = makeFrame?() else { return }
        dirty = false
        sequence &+= 1
        let maxWrite = peripheral.maximumWriteValueLength(for: .withResponse)
        queue = Wire.chunks(Wire.encode(frame), sequence: sequence, maxWrite: maxWrite).map { Data($0) }
        writeNext(to: snapshot)
    }

    private func writeNext(to characteristic: CBCharacteristic) {
        guard let peripheral, !queue.isEmpty else {
            writing = false
            if dirty { flush() }
            return
        }
        writing = true
        peripheral.writeValue(queue.removeFirst(), for: characteristic, type: .withResponse)
    }
}

// The manager delivers on the main queue, so the delegate conformances are main-actor isolated.
extension ClockLink: @MainActor CBCentralManagerDelegate {
    func centralManagerDidUpdateState(_ central: CBCentralManager) {
        switch central.state {
        case .poweredOn: connectToPaired()
        case .unauthorized: status = .unauthorized
        default: status = .bluetoothOff
        }
    }

    func centralManager(_ central: CBCentralManager, didDiscover peripheral: CBPeripheral,
                                    advertisementData: [String: Any], rssi RSSI: NSNumber) {
        let name = advertisementData[CBAdvertisementDataLocalNameKey] as? String ?? peripheral.name ?? "desk"
        let id = peripheral.identifier
        let rssi = RSSI.intValue
        if let index = nearby.firstIndex(where: { $0.id == id }) {
            nearby[index] = Nearby(id: id, name: name, rssi: rssi)
        } else {
            nearby.append(Nearby(id: id, name: name, rssi: rssi))
        }
    }

    func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        peripheral.discoverServices([serviceUUID])
    }

    func centralManager(_ central: CBCentralManager, didFailToConnect peripheral: CBPeripheral,
                                    error: Error?) {
        _ = Timer.scheduledTimer(withTimeInterval: 2, repeats: false) { [weak self] _ in
            MainActor.assumeIsolated { self?.connectToPaired() }
        }
    }

    func centralManager(_ central: CBCentralManager, didDisconnectPeripheral peripheral: CBPeripheral,
                                    error: Error?) {
        snapshot = nil
        events = nil
        queue = []
        writing = false
        if pairedID != nil {
            status = .searching
            central.connect(peripheral)
        }
    }
}

extension ClockLink: @MainActor CBPeripheralDelegate {
    func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        guard let service = peripheral.services?.first(where: { $0.uuid == serviceUUID }) else { return }
        peripheral.discoverCharacteristics([snapshotUUID, eventsUUID], for: service)
    }

    func peripheral(_ peripheral: CBPeripheral, didDiscoverCharacteristicsFor service: CBService,
                                error: Error?) {
        for characteristic in service.characteristics ?? [] {
            if characteristic.uuid == snapshotUUID { snapshot = characteristic }
            if characteristic.uuid == eventsUUID { events = characteristic }
        }
        // Subscribing to the encrypted characteristic is what makes macOS ask for the passkey
        // the first time.
        if let events { peripheral.setNotifyValue(true, for: events) }
        dirty = true
        flush()
    }

    func peripheral(_ peripheral: CBPeripheral, didWriteValueFor characteristic: CBCharacteristic,
                                error: Error?) {
        let failed = error != nil
        if failed {
            // Most likely pairing still in progress: start the frame over once it settles.
            queue = []
            writing = false
            dirty = true
            debounce?.invalidate()
            debounce = Timer.scheduledTimer(withTimeInterval: 1, repeats: false) { [weak self] _ in
                MainActor.assumeIsolated { self?.flush() }
            }
            return
        }
        if case .connected = status {} else {
            status = .connected(name: peripheral.name ?? "desk")
        }
        writeNext(to: characteristic)
    }

    func peripheral(_ peripheral: CBPeripheral, didUpdateValueFor characteristic: CBCharacteristic,
                                error: Error?) {
        guard let value = characteristic.value else { return }
        let bytes = [UInt8](value)
        if let command = Wire.decodeCommand(bytes) { onCommand?(command) }
    }
}
