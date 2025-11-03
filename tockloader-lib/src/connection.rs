use std::time::Duration;

use async_trait::async_trait;
use probe_rs::probe::DebugProbeInfo;
use probe_rs::{Permissions, Session};
use tokio::io::AsyncWriteExt;
use tokio_serial::{FlowControl, Parity, SerialPort, SerialStream, StopBits};

use crate::board_settings::BoardSettings;
use crate::errors::TockloaderError;
use log::info;
pub struct ProbeTargetInfo {
    pub chip: String,
    pub core: usize,
}

impl ProbeTargetInfo {
    pub fn default(chip: String) -> Self {
        Self { chip, core: 0 }
    }
}

pub struct SerialTargetInfo {
    pub baud_rate: u32,
    pub parity: Parity,
    pub stop_bits: StopBits,
    pub flow_control: FlowControl,
    pub timeout: Duration,
    pub request_to_send: bool,
    pub data_terminal_ready: bool,
}

impl Default for SerialTargetInfo {
    fn default() -> Self {
        Self {
            baud_rate: 115200,
            parity: Parity::None,
            stop_bits: StopBits::One,
            flow_control: FlowControl::None,
            timeout: Duration::from_millis(500),
            request_to_send: false,
            data_terminal_ready: false,
        }
    }
}

#[async_trait]
pub trait Connection {
    async fn open(&mut self) -> Result<(), TockloaderError>;
    /// Closes the connection, if it is open. If it is not open, it does
    /// nothing. On error the state of the connection is unknown and calling
    /// `open` or any other method is undefined behavior.
    async fn close(&mut self) -> Result<(), TockloaderError>;
    fn is_open(&self) -> bool;
    fn get_settings(&self) -> &BoardSettings;
}

pub struct ProbeRSConnection {
    pub(crate) session: Option<Session>,
    /// Used both to open new conections but also used during the session to
    /// provide information about the target
    pub(crate) target_info: ProbeTargetInfo,
    /// Only used for opening a new connection
    debug_probe: DebugProbeInfo,
    settings: BoardSettings,
}

impl ProbeRSConnection {
    pub fn new(
        debug_probe: DebugProbeInfo,
        target_info: ProbeTargetInfo,
        settings: BoardSettings,
    ) -> Self {
        Self {
            session: None,
            target_info,
            debug_probe,
            settings,
        }
    }
}

#[async_trait]
impl Connection for ProbeRSConnection {
    async fn open(&mut self) -> Result<(), TockloaderError> {
        let probe = self.debug_probe.open()?;

        self.session = Some(probe.attach(&self.target_info.chip, Permissions::default())?);
        info!("ProbeRS connection opened succesfully.");

        Ok(())
    }

    async fn close(&mut self) -> Result<(), TockloaderError> {
        // Session implements Drop, so we don't need to explicitly close it.
        self.session = None;
        info!("ProbeRS connection closed.");
        Ok(())
    }

    fn is_open(&self) -> bool {
        self.session.is_some()
    }

    fn get_settings(&self) -> &BoardSettings {
        &self.settings
    }
}

pub struct SerialConnection {
    pub(crate) stream: Option<SerialStream>,
    /// Used both to open new connections but also used during the session to
    /// provide information about the target
    pub(crate) target_info: SerialTargetInfo,
    /// Path to the serial port. This is only used for opening a new connection.
    port: String,
    settings: BoardSettings,
}

impl SerialConnection {
    pub fn new(port: String, target_info: SerialTargetInfo, settings: BoardSettings) -> Self {
        Self {
            stream: None,
            target_info,
            port,
            settings,
        }
    }
    pub fn into_inner_stream(self) -> Option<SerialStream> {
        self.stream
    }
}

#[async_trait]
impl Connection for SerialConnection {
    async fn open(&mut self) -> Result<(), TockloaderError> {
        let builder = tokio_serial::new(&self.port, self.target_info.baud_rate)
            .parity(self.target_info.parity)
            .stop_bits(self.target_info.stop_bits)
            .flow_control(self.target_info.flow_control)
            .timeout(self.target_info.timeout);

        let mut stream = SerialStream::open(&builder)?;

        stream.write_request_to_send(self.target_info.request_to_send)?;
        stream.write_data_terminal_ready(self.target_info.data_terminal_ready)?;

        self.stream = Some(stream);
        info!("Serial connection opened succesfully.");
        Ok(())
    }

    async fn close(&mut self) -> Result<(), TockloaderError> {
        if let Some(mut stream) = self.stream.take() {
            stream
                .shutdown()
                .await
                .map_err(|e| TockloaderError::Serial(e.into()))?;
        }
        info!("Serial connection closed.");
        Ok(())
    }

    fn is_open(&self) -> bool {
        self.stream.is_some()
    }

    fn get_settings(&self) -> &BoardSettings {
        &self.settings
    }
}

/// This is an utility enum to make your life easier when you want to abstract
/// away the underlying connection type. Use with caution, not all connection
/// types must implement every command.
#[allow(clippy::large_enum_variant)]
pub enum TockloaderConnection {
    ProbeRS(ProbeRSConnection),
    Serial(SerialConnection),
}

impl From<ProbeRSConnection> for TockloaderConnection {
    fn from(conn: ProbeRSConnection) -> Self {
        TockloaderConnection::ProbeRS(conn)
    }
}

impl From<SerialConnection> for TockloaderConnection {
    fn from(conn: SerialConnection) -> Self {
        TockloaderConnection::Serial(conn)
    }
}
#[async_trait]
impl Connection for TockloaderConnection {
    async fn open(&mut self) -> Result<(), TockloaderError> {
        match self {
            TockloaderConnection::ProbeRS(conn) => conn.open().await,
            TockloaderConnection::Serial(conn) => conn.open().await,
        }
    }

    async fn close(&mut self) -> Result<(), TockloaderError> {
        match self {
            TockloaderConnection::ProbeRS(conn) => conn.close().await,
            TockloaderConnection::Serial(conn) => conn.close().await,
        }
    }

    fn is_open(&self) -> bool {
        match self {
            TockloaderConnection::ProbeRS(conn) => conn.is_open(),
            TockloaderConnection::Serial(conn) => conn.is_open(),
        }
    }

    fn get_settings(&self) -> &BoardSettings {
        match self {
            TockloaderConnection::ProbeRS(conn) => conn.get_settings(),
            TockloaderConnection::Serial(conn) => conn.get_settings(),
        }
    }
}
