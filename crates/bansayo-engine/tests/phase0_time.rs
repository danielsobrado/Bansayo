use bansayo_contracts::TimeQuality;
use bansayo_engine::clock::SourceTime;
#[test]
fn device_clocks_use_minimum_offset_while_receive_clocks_subtract_latency() {
    let mut clock = SourceTime::default();
    assert_eq!(
        clock.map(Some(100), 1100, 0),
        (1100, TimeQuality::Estimated)
    );
    assert_eq!(
        clock.map(Some(200), 1250, 0),
        (1200, TimeQuality::Estimated)
    );
    assert_eq!(clock.map(None, 2000, 300), (1700, TimeQuality::ReceiveTime));
    assert_eq!(clock.map(None, 20, 300), (0, TimeQuality::ReceiveTime));
}
