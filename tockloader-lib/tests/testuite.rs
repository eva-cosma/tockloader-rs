use regression_test::RegTest;
use regression_test_macros::regtest;
use tockloader_lib::{
    connection::{Connection, ProbeRSConnection, TockloaderConnection},
    known_boards::{KnownBoard, Nrf52840dk},
    list_debug_probes, CommandList,
};

#[regtest]
#[tokio::test]
pub async fn test_list(mut rt: RegTest) {
    let debug_probes = list_debug_probes();

    dbg!(&debug_probes);
    assert!(
        debug_probes.len() == 1,
        "Expected exactly one debug probe to be connected, but found {}.",
        debug_probes.len()
    );

    let board = Nrf52840dk;

    let mut conn: TockloaderConnection = ProbeRSConnection::new(
        debug_probes[0].clone(),
        board.probe_target_info(),
        board.get_settings(),
    )
    .into();

    conn.open().await.expect("Failed to open connection.");

    let apps = conn.list().await.expect("Failed to list apps.");

    assert!(
        apps.len() == 1,
        "Expected exactly one app to be installed, but found {}.",
        apps.len()
    );

    assert_eq!(
        apps[0].tbf_header.get_package_name(),
        Some("c_hello"),
        "Expected the installed app to be 'c_hello', but found '{:?}'.",
        apps[0].tbf_header.get_package_name()
    );

    rt.regtest_dbg(apps);

    conn.close().await.expect("Failed to close connection.");
}
