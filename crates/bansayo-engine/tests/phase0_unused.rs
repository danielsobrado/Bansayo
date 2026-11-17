use bansayo_contracts::*;
use bansayo_engine::pipeline::features::unconsumed;
#[test]
fn validated_but_unused_sensor_capabilities_have_explicit_reasons() {
    for capability in [
        SensorCapability::Position,
        SensorCapability::Distance,
        SensorCapability::Contact,
        SensorCapability::Acceleration,
        SensorCapability::AngularVelocity,
    ] {
        assert_eq!(unconsumed(capability).unwrap().code, "not_implemented");
    }
    assert!(unconsumed(SensorCapability::Orientation).is_none());
    assert!(unconsumed(SensorCapability::Landmarks2d).is_none());
}
