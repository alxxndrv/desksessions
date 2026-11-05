//! What survives a reboot, in the `desk` partition (see `partitions.csv`): the device settings (so
//! the picture is the right way up before the Mac connects) and the Mac's bond.

// The demo build has no Bluetooth, so nothing asks for the bond.
#![cfg_attr(feature = "demo", allow(dead_code))]

use core::ops::Range;

use desk_core::app::DeviceSettings;
use embassy_embedded_hal::adapter::BlockingAsync;
use embedded_storage::nor_flash::NorFlash;
use esp_storage::FlashStorage;
use sequential_storage::cache::{Cache, Uncached};
use sequential_storage::map::{MapConfig, MapStorage};
use trouble_host::prelude::{
    AddrKind, Address, BdAddr, BondInformation, Identity, IdentityResolvingKey, LongTermKey,
    SecurityLevel,
};

const SETTINGS: u8 = 1;
const BOND: u8 = 2;
/// The `desk` data partition in `partitions.csv`.
const PARTITION: Range<u32> = 0x7F_0000..0x80_0000;
const BOND_BYTES: usize = 16 + 1 + 6 + 16 + 1 + 1;

type Flash = BlockingAsync<FlashStorage<'static>>;

pub struct Store {
    map: MapStorage<u8, Flash, Cache<Uncached, Uncached, Uncached, u8>>,
    buffer: [u8; 128],
}

fn settings_bytes(s: &DeviceSettings) -> [u8; 4] {
    [u8::from(s.flip_screen), u8::from(s.swap_buttons), s.day_brightness, s.night_brightness]
}

fn bond_bytes(bond: &BondInformation) -> [u8; BOND_BYTES] {
    let mut out = [0; BOND_BYTES];
    out[..16].copy_from_slice(&bond.ltk.0.to_le_bytes());
    out[16] = bond.identity.addr.kind.into_inner();
    out[17..23].copy_from_slice(bond.identity.addr.addr.raw());
    if let Some(irk) = bond.identity.irk {
        out[23..39].copy_from_slice(&irk.0.get().to_le_bytes());
    }
    out[39] = u8::from(bond.is_bonded);
    out[40] = match bond.security_level {
        SecurityLevel::NoEncryption => 0,
        SecurityLevel::Encrypted => 1,
        SecurityLevel::EncryptedAuthenticated => 2,
    };
    out
}

fn bond_from(bytes: &[u8]) -> Option<BondInformation> {
    if bytes.len() != BOND_BYTES {
        return None;
    }
    let u128_at = |at: usize| u128::from_le_bytes(bytes[at..at + 16].try_into().unwrap_or([0; 16]));
    let mut addr = [0; 6];
    addr.copy_from_slice(&bytes[17..23]);
    Some(BondInformation {
        ltk: LongTermKey(u128_at(0)),
        identity: Identity {
            addr: Address { kind: AddrKind::new(bytes[16]), addr: BdAddr::new(addr) },
            irk: IdentityResolvingKey::new(u128_at(23)),
        },
        is_bonded: bytes[39] != 0,
        security_level: match bytes[40] {
            0 => SecurityLevel::NoEncryption,
            1 => SecurityLevel::Encrypted,
            _ => SecurityLevel::EncryptedAuthenticated,
        },
    })
}

impl Store {
    #[must_use]
    pub fn new(flash: FlashStorage<'static>) -> Self {
        let range = if (flash.capacity() as u32) >= PARTITION.end {
            PARTITION
        } else {
            // A smaller flash than the T190's: the last four sectors.
            let end = flash.capacity() as u32;
            end - <FlashStorage<'static> as NorFlash>::ERASE_SIZE as u32 * 4..end
        };
        Self {
            map: MapStorage::new(
                BlockingAsync::new(flash),
                MapConfig::new(range),
                Cache::new_uncached(),
            ),
            buffer: [0; 128],
        }
    }

    pub async fn settings(&mut self) -> DeviceSettings {
        match self.map.fetch_item::<&[u8]>(&mut self.buffer, &SETTINGS).await {
            Ok(Some(&[flip, swap, day, night])) => DeviceSettings {
                flip_screen: flip != 0,
                swap_buttons: swap != 0,
                day_brightness: day.min(100),
                night_brightness: night.min(100),
            },
            _ => DeviceSettings::default(),
        }
    }

    pub async fn save_settings(&mut self, settings: &DeviceSettings) {
        let bytes = settings_bytes(settings);
        if let Err(e) = self.map.store_item(&mut self.buffer, &SETTINGS, &&bytes[..]).await {
            esp_println::println!("[store] settings not saved: {e:?}");
        }
    }

    pub async fn bond(&mut self) -> Option<BondInformation> {
        match self.map.fetch_item::<&[u8]>(&mut self.buffer, &BOND).await {
            Ok(Some(bytes)) => bond_from(bytes),
            _ => None,
        }
    }

    pub async fn save_bond(&mut self, bond: &BondInformation) {
        let bytes = bond_bytes(bond);
        if let Err(e) = self.map.store_item(&mut self.buffer, &BOND, &&bytes[..]).await {
            esp_println::println!("[store] bond not saved: {e:?}");
        }
    }

    pub async fn forget_bond(&mut self) {
        if let Err(e) = self.map.remove_item(&mut self.buffer, &BOND).await {
            esp_println::println!("[store] bond not erased: {e:?}");
        }
    }
}
