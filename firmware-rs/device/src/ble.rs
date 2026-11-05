//! The BLE peripheral: one GATT service the Mac writes snapshot chunks to and listens on for the
//! clock's commands. Bonded, MITM-protected LE Secure Connections; the clock shows a passkey the
//! Mac types in. One Mac per clock.
//!
//! The BLE task never touches app state: what it learns goes to the UI task through [`INBOX`],
//! and commands come back through [`OUTBOX`].

use core::fmt::Write as _;

use desk_core::app::{LinkEvent, MacLink};
use desk_core::domain::Command;
use desk_core::protocol::encode_command;
use embassy_futures::join::join;
use embassy_futures::select::{Either3, select3};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use static_cell::StaticCell;
use trouble_host::prelude::*;

use crate::storage::Store;

const SERVICE: &str = "b4d4f873-8133-48f0-9479-88481c6d52cf";

/// Longest chunk the Mac writes.
pub const CHUNK_BYTES: usize = 512;

/// Something the BLE task learned, for the UI task. Chunks travel by value through a short queue,
/// as they did in the C++ firmware; boxing them would put the BLE task at the allocator's mercy.
#[allow(clippy::large_enum_variant)]
pub enum LinkMessage {
    Chunk(heapless::Vec<u8, CHUNK_BYTES>),
    Event(LinkEvent),
}

/// BLE task → UI task. If the UI has fallen this far behind, dropping beats blocking BLE.
pub static INBOX: Channel<CriticalSectionRawMutex, LinkMessage, 12> = Channel::new();
/// UI task → BLE task: commands for the Mac.
static OUTBOX: Channel<CriticalSectionRawMutex, Command, 4> = Channel::new();
/// UI task → BLE task: wipe the bond and pair again.
static FORGET: Signal<CriticalSectionRawMutex, ()> = Signal::new();

fn post(message: LinkMessage) {
    let _ = INBOX.try_send(message);
}

/// The UI side of the link.
pub struct UiLink;

impl MacLink for UiLink {
    fn send(&mut self, command: Command) {
        let _ = OUTBOX.try_send(command);
    }

    fn forget_pairing(&mut self) {
        FORGET.signal(());
    }
}

#[gatt_server]
struct Server {
    desk: DeskService,
}

#[gatt_service(uuid = "b4d4f873-8133-48f0-9479-88481c6d52cf")]
struct DeskService {
    /// Snapshot chunks from the Mac.
    #[characteristic(uuid = "8c9ccb7b-95a9-411e-a3db-00b974bbb1ea", write, permissions(write = authenticated))]
    snapshot: heapless::Vec<u8, CHUNK_BYTES>,
    /// Commands for the Mac: dismiss, jump.
    #[characteristic(uuid = "6fbc6994-1373-4ace-9710-e9bf939d9251", read, notify, permissions(read = authenticated, cccd = authenticated))]
    events: [u8; 5],
}

/// "desk-23D9": the last two bytes of the Bluetooth address, as the C++ firmware named it.
fn device_name(address: &[u8; 6]) -> &'static str {
    static NAME: StaticCell<heapless::String<16>> = StaticCell::new();
    let name = NAME.init(heapless::String::new());
    let _ = write!(name, "desk-{:02X}{:02X}", address[4], address[5]);
    name.as_str()
}

/// Runs the BLE stack forever. `address` is the Bluetooth MAC, in display order.
pub async fn run<C: Controller>(
    controller: C,
    address: [u8; 6],
    store: &'static Mutex<CriticalSectionRawMutex, Store>,
) {
    let mut resources: HostResources<C, DefaultPacketPool, 1, 3> = HostResources::new();
    let stack = trouble_host::new(controller, &mut resources)
        .set_io_capabilities(IoCapabilities::DisplayOnly)
        .build();

    let mut bonded = store.lock().await.bond().await;
    if let Some(bond) = bonded.clone()
        && stack.add_bond_information(bond).is_err()
    {
        bonded = None;
    }
    post(idle(&bonded));

    let name = device_name(&address);
    let Ok(server) = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name,
        appearance: &appearance::UNKNOWN,
    })) else {
        esp_println::println!("[ble] GATT server failed");
        return;
    };
    let mut runner = stack.runner();
    let mut peripheral = stack.peripheral();

    let host = async {
        loop {
            if let Err(e) = runner.run().await {
                esp_println::println!("[ble] host error: {e:?}");
            }
        }
    };
    let app = async {
        loop {
            let conn = match advertise(name, &mut peripheral, &server).await {
                Ok(conn) => conn,
                Err(e) => {
                    esp_println::println!("[ble] advertising failed: {e:?}");
                    embassy_time::Timer::after_secs(1).await;
                    continue;
                }
            };
            let _ = conn.raw().set_bondable(true);
            // Ask for encryption straight away: pairing if new, re-encryption if bonded.
            let _ = conn.raw().request_security();
            match select3(
                events(&conn, &stack, store, &mut bonded),
                send_commands(&server, &conn),
                FORGET.wait(),
            )
            .await
            {
                Either3::Third(()) => {
                    forget(&stack, store, &mut bonded).await;
                    conn.raw().disconnect();
                    post(LinkMessage::Event(LinkEvent::Unpaired));
                    continue;
                }
                Either3::First(()) | Either3::Second(()) => {}
            }
            post(idle(&bonded));
        }
    };
    join(host, app).await;
}

/// What the link is while nobody is connected: waiting for the bonded Mac, or for a new one.
fn idle(bonded: &Option<BondInformation>) -> LinkMessage {
    LinkMessage::Event(if bonded.is_some() { LinkEvent::Disconnected } else { LinkEvent::Unpaired })
}

async fn forget<C: Controller>(
    stack: &Stack<'_, C, DefaultPacketPool>,
    store: &Mutex<CriticalSectionRawMutex, Store>,
    bonded: &mut Option<BondInformation>,
) {
    if let Some(bond) = bonded.take() {
        let _ = stack.remove_bond_information(bond.identity);
    }
    store.lock().await.forget_bond().await;
}

/// Handles one connection's events until it closes.
async fn events<C: Controller>(
    conn: &GattConnection<'_, '_, DefaultPacketPool>,
    stack: &Stack<'_, C, DefaultPacketPool>,
    store: &Mutex<CriticalSectionRawMutex, Store>,
    bonded: &mut Option<BondInformation>,
) {
    loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => {
                esp_println::println!("[ble] disconnected: {reason:?}");
                return;
            }
            GattConnectionEvent::PassKeyDisplay(key) => {
                post(LinkMessage::Event(LinkEvent::Passkey(key.value())));
            }
            // A fresh pairing: the link is encrypted first, then keys are exchanged and this
            // arrives. Only a bonded, MITM-protected pairing counts.
            GattConnectionEvent::PairingComplete { security_level, bond } => {
                let Some(bond) = bond else {
                    conn.raw().disconnect(); // only bonded Macs
                    continue;
                };
                // One Mac per clock: a second bond means someone else paired. Undo it.
                if let Some(existing) = bonded.as_ref()
                    && existing.identity != bond.identity
                {
                    let _ = stack.remove_bond_information(bond.identity);
                    let _ = stack.add_bond_information(existing.clone());
                    conn.raw().disconnect();
                    continue;
                }
                store.lock().await.save_bond(&bond).await;
                *bonded = Some(bond);
                if security_level == SecurityLevel::EncryptedAuthenticated {
                    post(LinkMessage::Event(LinkEvent::Connected));
                } else {
                    conn.raw().disconnect();
                }
            }
            GattConnectionEvent::PairingFailed(e) => {
                esp_println::println!("[ble] pairing failed: {e:?}");
                conn.raw().disconnect();
            }
            // The bonded Mac came back and the link is encrypted with its stored key. (During a
            // fresh pairing this arrives without a bond, before PairingComplete: wait for that.)
            GattConnectionEvent::Encrypted { security_level, bond: Some(_) } => {
                if security_level == SecurityLevel::EncryptedAuthenticated {
                    post(LinkMessage::Event(LinkEvent::Connected));
                } else {
                    conn.raw().disconnect();
                }
            }
            GattConnectionEvent::Gatt { event } => {
                if let GattEvent::Write(write) = &event {
                    write.with_data(|_, data| {
                        if let Ok(chunk) = heapless::Vec::from_slice(data) {
                            post(LinkMessage::Chunk(chunk));
                        }
                    });
                }
                match event.accept() {
                    Ok(reply) => reply.send().await,
                    Err(e) => esp_println::println!("[ble] reply failed: {e:?}"),
                }
            }
            _ => {}
        }
    }
}

/// Forwards the UI's commands to the Mac as notifications.
async fn send_commands(server: &Server<'_>, conn: &GattConnection<'_, '_, DefaultPacketPool>) {
    loop {
        let command = OUTBOX.receive().await;
        if server.desk.events.notify(conn, &encode_command(&command), false).await.is_err() {
            return;
        }
    }
}

async fn advertise<'values, 'server, C: Controller>(
    name: &'values str,
    peripheral: &mut Peripheral<'values, C, DefaultPacketPool>,
    server: &'server Server<'values>,
) -> Result<GattConnection<'values, 'server, DefaultPacketPool>, BleHostError<C::Error>> {
    let service = Uuid::new_long(uuid_bytes(SERVICE));
    let mut adv = [0; 31];
    let adv_len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::IncompleteServiceUuids128(&[service
                .as_raw()
                .try_into()
                .unwrap_or([0; 16])]),
        ],
        &mut adv[..],
    )?;
    let mut scan = [0; 31];
    let scan_len = AdStructure::encode_slice(
        &[AdStructure::CompleteLocalName(name.as_bytes())],
        &mut scan[..],
    )?;
    let advertiser = peripheral
        .advertise(
            &AdvertisementParameters::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &adv[..adv_len],
                scan_data: &scan[..scan_len],
            },
        )
        .await?;
    Ok(advertiser.accept().await?.with_attribute_server(server)?)
}

/// A UUID string as the little-endian bytes BLE sends.
fn uuid_bytes(text: &str) -> [u8; 16] {
    let mut out = [0u8; 16];
    let mut i = 0;
    let hex = |c: u8| match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0,
    };
    let digits: heapless::Vec<u8, 32> = text.bytes().filter(u8::is_ascii_hexdigit).collect();
    while i < 16 {
        out[15 - i] = hex(digits[2 * i]) << 4 | hex(digits[2 * i + 1]);
        i += 1;
    }
    out
}
