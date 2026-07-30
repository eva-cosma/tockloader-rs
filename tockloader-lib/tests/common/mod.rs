use tockloader_lib::{
    connection::{Connection, ProbeRSConnection, TockloaderConnection},
    known_boards::{KnownBoard, MicrobitV2},
    list_debug_probes,
};

pub const C_HELLO_PACKAGE_NAME: &str = "c_hello";

pub fn c_hello_tab_path() -> String {
    format!("{}/../test_data/c_hello.tab", env!("CARGO_MANIFEST_DIR"))
}

/// Opens a probe-rs connection to the (single) connected micro:bit v2.
pub async fn connect_microbit() -> TockloaderConnection {
    let debug_probes = list_debug_probes();
    dbg!(&debug_probes);
    assert!(
        debug_probes.len() == 1,
        "Expected exactly one debug probe to be connected, but found {}.",
        debug_probes.len()
    );

    let board = MicrobitV2;
    let mut conn: TockloaderConnection = ProbeRSConnection::new(
        debug_probes[0].clone(),
        board.probe_target_info(),
        board.get_settings(),
    )
    .into();

    conn.open().await.expect("Failed to open connection.");
    conn
}
