
use std::fmt;

use std::pin::Pin;
use btleplug::api::CentralEvent;
use btleplug::api::{Central, Manager as _, ScanFilter, Peripheral, Characteristic};
use btleplug::platform::{Adapter, Manager, PeripheralId};
use futures::{future, Stream, StreamExt};

use characteristics as ch;

pub fn temperature_c_to_f(c: f32) -> f32 { c * 1.8 + 32.0 }
pub fn pressure_hpa_to_atm(hpa: f32) -> f32 { hpa/1013.25 }

pub mod uuids {
    use uuid::{uuid, Uuid};


    // Aranet UUIDs and handles
    // https://github.com/Anrijs/Aranet4-Python/blob/b712654891c6f434c04774cb62f8aea0d97016a5/aranet4/client.py#L261

    /// Manufacturer id for LE advertisement.
    /// BTLE Assigned Numbers document: https://btprodspecificationrefs.blob.core.windows.net/assigned-numbers/Assigned%20Number%20Types/Assigned_Numbers.pdf
    /// 0x0702 is assigned to 'Akciju sabiedriba "SAF TEHNIKA"', where SAF Tehnika is the parent company behind the 'Aranet' brand.
    pub const MANUFACTURER_ID: u16 = 0x0702;

    // Services
    pub const AR4_OLD_SERVICE: Uuid = uuid!("f0cd1400-95da-4f4b-9ac8-aa55d312af0c"); // until v1.2.0
    pub const AR4_SERVICE: Uuid = uuid!("0000fce0-0000-1000-8000-00805f9b34fb"); // v1.2.0 and later
    pub const GENERIC_SERVICE: Uuid = uuid!("00001800-0000-1000-8000-00805f9b34fb");
    pub const COMMON_SERVICE: Uuid = uuid!("0000180a-0000-1000-8000-00805f9b34fb");

    // Read / Aranet service
    pub const AR4_READ_CURRENT_READINGS: Uuid = uuid!("f0cd1503-95da-4f4b-9ac8-aa55d312af0c");
    pub const AR4_READ_CURRENT_READINGS_DET: Uuid = uuid!("f0cd3001-95da-4f4b-9ac8-aa55d312af0c");
    pub const AR4_READ_INTERVAL: Uuid = uuid!("f0cd2002-95da-4f4b-9ac8-aa55d312af0c");
    pub const AR4_READ_SECONDS_SINCE_UPDATE: Uuid = uuid!("f0cd2004-95da-4f4b-9ac8-aa55d312af0c");
    pub const AR4_READ_TOTAL_READINGS: Uuid = uuid!("f0cd2001-95da-4f4b-9ac8-aa55d312af0c");
    pub const AR4_READ_HISTORY_READINGS_V1: Uuid = uuid!("f0cd2003-95da-4f4b-9ac8-aa55d312af0c");
    pub const AR4_READ_HISTORY_READINGS_V2: Uuid = uuid!("f0cd2005-95da-4f4b-9ac8-aa55d312af0c");

    // Read / Generic servce
    pub const GENERIC_READ_DEVICE_NAME: Uuid = uuid!("00002a00-0000-1000-8000-00805f9b34fb");

    // Read / Common servce
    pub const COMMON_READ_MANUFACTURER_NAME: Uuid = uuid!("00002a29-0000-1000-8000-00805f9b34fb");
    pub const COMMON_READ_MODEL_NUMBER: Uuid = uuid!("00002a24-0000-1000-8000-00805f9b34fb");
    pub const COMMON_READ_SERIAL_NO: Uuid = uuid!("00002a25-0000-1000-8000-00805f9b34fb");
    pub const COMMON_READ_HW_REV: Uuid = uuid!("00002a27-0000-1000-8000-00805f9b34fb");
    pub const COMMON_READ_FACTORY_SW_REV: Uuid = uuid!("00002a28-0000-1000-8000-00805f9b34fb");
    pub const COMMON_READ_SW_REV: Uuid = uuid!("00002a26-0000-1000-8000-00805f9b34fb");
    // pub const COMMON_READ_BATTERY: Uuid = uuid!("00002a19-0000-1000-8000-00805f9b34fb");

    // Write / Aranet service
    pub const AR4_WRITE_CMD: Uuid = uuid!("f0cd1402-95da-4f4b-9ac8-aa55d312af0c");


    // found independantly of the aranet4-python library
    // https://gist.github.com/sam016/4abe921b5a9ee27f67b3686910293026
    pub const GENERIC_ATTRIBUTE: Uuid = uuid!("00001801-0000-1000-8000-00805f9b34fb");
    pub const BATTERY_SERVICE: Uuid = uuid!("0000180f-0000-1000-8000-00805f9b34fb");
    pub const BATTERY_READ: Uuid = uuid!("00002a19-0000-1000-8000-00805f9b34fb");
    pub const COMMON_APPEARANCE: Uuid = uuid!("00002a01-0000-1000-8000-00805f9b34fb");
    pub const COMMON_PREFERRED_CONNECT_PARAMS: Uuid = uuid!("00002a04-0000-1000-8000-00805f9b34fb");
    pub const GENERIC_SVC_CHANGED: Uuid = uuid!("00002a05-0000-1000-8000-00805f9b34fb");
    pub const COMMON_SYSTEM_ID: Uuid = uuid!("00002a23-0000-1000-8000-00805f9b34fb");
    pub const COMMON_CENTRAL_ADDR_RESOLUTION: Uuid = uuid!("00002aa6-0000-1000-8000-00805f9b34fb");

    // https://github.com/ghostyguo/BleUuidExplorer/blob/master/app/src/main/java/ghostysoft/bleuuidexplorer/GattAttributes.java
    pub const MANUF_NORDIC_SEMICONDUCTOR_ASA: Uuid = uuid!("0000fe59-0000-1000-8000-00805f9b34fb");

    // https://github.com/Anrijs/Aranet4-Python/blob/b712654891c6f434c04774cb62f8aea0d97016a5/docs/UUIDs.md?plain=1#L12
    pub const AR4_READ_SENSOR_CALIBRATION: Uuid = uuid!("f0cd1502-95da-4f4b-9ac8-aa55d312af0c");
    pub const AR4_READ_SENSOR_SETTINGS: Uuid = uuid!("f0cd1401-95da-4f4b-9ac8-aa55d312af0c");

    /// device firmware update
    pub const NORDIC_DFU: Uuid = uuid!("8ec90003-f315-4f60-9fb8-838830daea50");
}

pub mod characteristics {
    use btleplug::api::{Characteristic, CharPropFlags};

    use crate::uuids;

    macro_rules! characteristic {
        ($svc: ident, $name: ident, $flags: expr) => {
            pub const $name: Characteristic = Characteristic {
                uuid: uuids::$name,
                service_uuid: uuids::$svc,
                properties: $flags
            };
        };
        ($svc: ident, $name: ident) => {
            characteristic!($svc, $name, CharPropFlags::READ);
        };
    }

    // scraped from a bluetooth characistic dump on firmware v1.2.0, hw rev 12, in dumped order (not sure if that's deterministic)
    characteristic!(GENERIC_SERVICE,   GENERIC_READ_DEVICE_NAME);
    characteristic!(GENERIC_SERVICE,   COMMON_APPEARANCE);
    characteristic!(GENERIC_SERVICE,   COMMON_PREFERRED_CONNECT_PARAMS);
    characteristic!(GENERIC_ATTRIBUTE, GENERIC_SVC_CHANGED, CharPropFlags::INDICATE);
    characteristic!(BATTERY_SERVICE,   BATTERY_READ, CharPropFlags::READ.union(CharPropFlags::NOTIFY));
    characteristic!(COMMON_SERVICE,    COMMON_SYSTEM_ID);
    characteristic!(COMMON_SERVICE,    COMMON_READ_MODEL_NUMBER);
    characteristic!(COMMON_SERVICE,    COMMON_READ_SERIAL_NO);
    characteristic!(COMMON_SERVICE,    COMMON_READ_SW_REV);
    characteristic!(COMMON_SERVICE,    COMMON_READ_HW_REV);
    characteristic!(COMMON_SERVICE,    COMMON_READ_FACTORY_SW_REV);
    characteristic!(COMMON_SERVICE,    COMMON_READ_MANUFACTURER_NAME);
    characteristic!(GENERIC_SERVICE,   COMMON_CENTRAL_ADDR_RESOLUTION);
    characteristic!(MANUF_NORDIC_SEMICONDUCTOR_ASA, NORDIC_DFU, CharPropFlags::WRITE.union(CharPropFlags::INDICATE));
    characteristic!(AR4_SERVICE,       AR4_READ_SENSOR_SETTINGS);
    characteristic!(AR4_SERVICE,       AR4_WRITE_CMD, CharPropFlags::WRITE);
    characteristic!(AR4_SERVICE,       AR4_READ_SENSOR_CALIBRATION);
    characteristic!(AR4_SERVICE,       AR4_READ_CURRENT_READINGS);
    characteristic!(AR4_SERVICE,       AR4_READ_TOTAL_READINGS);
    characteristic!(AR4_SERVICE,       AR4_READ_INTERVAL);
    characteristic!(AR4_SERVICE,       AR4_READ_HISTORY_READINGS_V1, CharPropFlags::READ.union(CharPropFlags::NOTIFY));
    characteristic!(AR4_SERVICE,       AR4_READ_SECONDS_SINCE_UPDATE);
    characteristic!(AR4_SERVICE,       AR4_READ_HISTORY_READINGS_V2);
    characteristic!(AR4_SERVICE,       AR4_READ_CURRENT_READINGS_DET);


}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Version {
    major: u8,
    minor: u8,
    patch: u8,
}
impl Version {
    pub fn new(major: u8, minor: u8, patch: u8) -> Version {
        Version { major, minor, patch }
    }
}
impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}.{}.{}", self.major, self.minor, self.patch)
    }
}
impl fmt::Debug for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CalibrationState {
    NotActive = 0,
    EndRequest = 1,
    InProgress = 2,
    Error = 3,
}
impl CalibrationState {
    pub fn from_raw(b: u8) -> Option<CalibrationState> {
        Some(match b {
            0 => CalibrationState::NotActive,
            1 => CalibrationState::EndRequest,
            2 => CalibrationState::InProgress,
            3 => CalibrationState::Error,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DisplayStatus {
    Green = 1,
    Yellow = 2,
    Red = 3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurrentReading {
    /// in ppm
    pub co2_ppm: Option<u16>,
    /// in celcius
    pub temperature_c: Option<f32>,
    /// in hPa
    pub pressure_hpa: Option<f32>,
    /// from 0 to 1
    pub humidity: f32,
    /// from 0 to 1
    pub battery: f32,
    pub status: DisplayStatus,
}

impl CurrentReading {
    pub fn parse(data: [u8; 9]) -> CurrentReading {
        // reference for the filtering/mapped options:
        // https://github.com/Anrijs/Aranet4-Python/blob/b712654891c6f434c04774cb62f8aea0d97016a5/aranet4/client.py#L108

        let co2 = Some(u16::from_le_bytes([data[0], data[1]]))
            .filter(|r| r >> 15 != 1);
        let temperature = Some(u16::from_le_bytes([data[2], data[3]]))
            .filter(|r| ((r >> 14) & 1) != 1)
            .map(|r| r as f32 * 0.05);
        let pressure = Some(u16::from_le_bytes([data[4], data[5]]))
            .filter(|r| r >> 15 != 1)
            .map(|r| r as f32 * 0.1);
        let humidity = data[6] as f32 / 100.0;
        let battery = data[7] as f32 / 100.0;
        let status = match data[8] {
            1 => DisplayStatus::Green,
            2 => DisplayStatus::Yellow,
            3 => DisplayStatus::Red,
            o => panic!("unexpected display status value: {}", o),
        };

        CurrentReading { co2_ppm: co2, temperature_c: temperature, pressure_hpa: pressure, humidity, battery, status }
    }

    pub fn temperature_f(&self) -> Option<f32> {
        self.temperature_c
            .map(|c| c * 1.8 + 32.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurrentReadingDetailed {
    /// in ppm
    pub co2_ppm: Option<u16>,
    /// in celcius
    pub temperature_c: Option<f32>,
    /// in hPa
    pub pressure_hpa: Option<f32>,
    /// from 0 to 1
    pub humidity: f32,
    /// from 0 to 1
    pub battery: f32,
    pub status: DisplayStatus,
    /// in seconds
    pub interval: u16,
    /// in seconds
    pub age: u16,
}

impl CurrentReadingDetailed {
    pub fn parse(data: [u8; 13]) -> CurrentReadingDetailed {
        let cr = CurrentReading::parse(data[..9].try_into().unwrap());
        let CurrentReading { co2_ppm: co2, temperature_c: temperature, pressure_hpa: pressure, humidity, battery, status } = cr;

        let interval = u16::from_le_bytes([data[9], data[10]]);
        let age = u16::from_le_bytes([data[11], data[12]]);

        CurrentReadingDetailed { co2_ppm: co2, temperature_c: temperature, pressure_hpa: pressure, humidity, battery, status, interval, age }
    }
    
    pub fn temperature_f(&self) -> Option<f32> {
        self.temperature_c.map(temperature_c_to_f)
    }

    pub fn pressure_atm(&self) -> Option<f32> {
        self.pressure_hpa.map(pressure_hpa_to_atm)
    }
}

impl fmt::Display for CurrentReadingDetailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Measurement Age: {}/{}s", self.age, self.interval)?;
        writeln!(f, "Battery: {:.0}%", self.battery * 100.0)?;
        if let Some(ppm) = self.co2_ppm {
            writeln!(f, "CO2: {} PPM", ppm)?;
        }
        writeln!(f, "CO2 Status: {:?}", self.status)?;
        if let Some(c) = self.temperature_c {
            writeln!(f, "Temperature: {:.1}°F ({:.1}°C)", temperature_c_to_f(c), c)?;
        }
        writeln!(f, "Rel. Humidity: {:.0}%", self.humidity * 100.0)?;
        if let Some(hpa) = self.pressure_hpa {
            writeln!(f, "Pressure: {:.3} atm ({:.0} hPa)", pressure_hpa_to_atm(hpa), hpa)?;
        }
        
        Ok(())
    }
}

impl CurrentReading {
    pub const CHARACTERISTIC: Characteristic = characteristics::AR4_READ_CURRENT_READINGS;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManufacturerData {
    disconnected: bool,
    calibration_state: CalibrationState,
    dfu_active: bool,
    integrations: bool,
    version: Version,
}
impl ManufacturerData {
    pub fn parse(data: [u8; 7]) -> ManufacturerData {
        let disconnected = data[0] & (1 << 0) != 0;
        let calibration_state = CalibrationState::from_raw((data[0] >> 2) & 0x03).expect("unexpected value for calibration state");
        let dfu_active = data[0] & (1 << 4) != 0;
        let integrations = data[0] & (1 << 5) != 0;
        let version = Version::new(data[3], data[2], data[1]);
        ManufacturerData { disconnected, calibration_state, dfu_active, integrations, version }
    }
}

pub struct Aranet4<P: Peripheral> {
    device: P,
}

impl<P: Peripheral> Aranet4<P> {
    /// Creates a strongly typed Aranet4 peripheral. Will discover services if it has not already been done.
    pub async fn new(device: P) -> btleplug::Result<Self> {
        if ! device.is_connected().await? { return Err(btleplug::Error::NotConnected); }
        if device.services().is_empty() {
            device.discover_services().await?;
        }
        let is_aranet = device.services().iter().any(|u| u.uuid == uuids::AR4_SERVICE);
        if ! is_aranet {
            return Err(btleplug::Error::NotSupported("device is not an Aranet4 device (or firmware is not v1.2.0+)".to_owned()));
            
        }
        Ok(Aranet4 { device })
    }

    pub async fn current_readings(&self) -> btleplug::Result<CurrentReading> {
        if ! self.device.is_connected().await? { return Err(btleplug::Error::NotConnected); }
        let raw: Vec<u8> = self.device.read(&ch::AR4_READ_CURRENT_READINGS).await?;
        Ok(CurrentReading::parse(raw.try_into().expect("expected current readings to be a 9 byte array")))
    }

    pub async fn current_readings_details(&self) -> btleplug::Result<CurrentReadingDetailed> {
        if ! self.device.is_connected().await? { return Err(btleplug::Error::NotConnected); }
        let raw: Vec<u8> = self.device.read(&ch::AR4_READ_CURRENT_READINGS_DET).await?;
        Ok(CurrentReadingDetailed::parse(raw.try_into().expect("expected current readings (detailed) to be a 13 byte array")))
    }

    /// Interval between environment samples, in seconds
    pub async fn interval(&self) -> btleplug::Result<u16> {
        if ! self.device.is_connected().await? { return Err(btleplug::Error::NotConnected); }
        let raw = self.device.read(&ch::AR4_READ_INTERVAL).await?;

        Ok(u16::from_le_bytes(raw.try_into().expect("expected interval to be a 2-byte little endian integer")))
    }

    /// The name of the device.
    pub async fn name(&self) -> btleplug::Result<String> {
        if ! self.device.is_connected().await? { return Err(btleplug::Error::NotConnected); }
        let raw = self.device.read(&ch::GENERIC_READ_DEVICE_NAME).await?;

        Ok(String::from_utf8(raw).map_err(|e| btleplug::Error::Other(Box::new(e)))?)
    }

    /// The version string of the firmware
    pub async fn version(&self) -> btleplug::Result<String> {
        if ! self.device.is_connected().await? { return Err(btleplug::Error::NotConnected); }
        let raw = self.device.read(&ch::COMMON_READ_SW_REV).await?;

        Ok(String::from_utf8(raw).map_err(|e| btleplug::Error::Other(Box::new(e)))?)
    }

    /// The number of seconds since the last environment sample was taken
    pub async fn last_update_age(&self) -> btleplug::Result<u16> {
        if ! self.device.is_connected().await? { return Err(btleplug::Error::NotConnected); }
        let raw = self.device.read(&ch::AR4_READ_SECONDS_SINCE_UPDATE).await?;

        Ok(u16::from_le_bytes(raw.try_into().expect("expected last update age to be a 2-byte little endian integer")))
    }

    /// The number of seconds since the last environment sample was taken
    pub async fn total_readings(&self) -> btleplug::Result<u16> {
        if ! self.device.is_connected().await? { return Err(btleplug::Error::NotConnected); }
        let raw = self.device.read(&ch::AR4_READ_SECONDS_SINCE_UPDATE).await?;

        Ok(u16::from_le_bytes(raw.try_into().expect("expected total readings to be a 2-byte little endian integer")))
    }
}

#[derive(Debug, Clone)]
pub struct DiscoveredAranet {
    pub adapter: Adapter,
    pub peripheral_id: PeripheralId,
    pub manufacturer_data: ManufacturerData,
    pub current_reading: Option<CurrentReadingDetailed>,
}

/// Attempt to locate an Aranet4 device, by finding a device that advertises manufacturer data with the correct ID
pub async fn discover_aranet4(manager: &Manager) -> btleplug::Result<Pin<Box<impl Stream<Item = DiscoveredAranet>>>> {
    let adapters = manager.adapters().await?;
    log::debug!("Found {} BTLE adapters", adapters.len());
    let mut event_streams = Vec::with_capacity(adapters.len());

    for (adapter_idx, adapter) in adapters.into_iter().enumerate() {
        log::debug!("BTLE Adapter#{} - Found {:?}", adapter_idx, adapter);
        adapter.start_scan(ScanFilter { services: vec![uuids::AR4_SERVICE] }).await?;
        log::debug!("BTLE Adapter#{} - Started scanning", adapter_idx);
        let events = adapter.events().await?;
        log::debug!("BTLE Adapter#{} - Listening", adapter_idx);
        let inspected = events.inspect(move |ce| {
            log::trace!("BTLE Adapter#{} - Event {:?}", adapter_idx, ce);
        });
        event_streams.push(inspected.filter_map(move |ce| {
            future::ready(match ce {
                CentralEvent::ManufacturerDataAdvertisement { id, manufacturer_data } => {
                    if let Some(data) = manufacturer_data.get(&uuids::MANUFACTURER_ID) {
                        let raw_manuf = data[..7].try_into()
                            .expect("Aranet4's Manufacturer ID used for advertisement data under 7 bytes!");
                        let manufacturer_data = ManufacturerData::parse(raw_manuf);
                        let current_reading = data[8..21].try_into()
                            .map(CurrentReadingDetailed::parse)
                            .ok();
                        Some(DiscoveredAranet {
                            adapter: adapter.clone(),
                            peripheral_id: id,
                            manufacturer_data,
                            current_reading,
                        })
                    } else {
                        /* unknown manufacturer ID */
                        None
                    }
                },
                _ => {
                    /* other discovery methods may be implemented in the future, for now - just manufacturer data */
                    None
                }
            })
        }));
    }

    log::debug!("listening on {} BTLE adapters", event_streams.len());
    Ok(Box::pin(futures::stream::select_all(event_streams)))
}
