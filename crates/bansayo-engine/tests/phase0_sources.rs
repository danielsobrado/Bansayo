use bansayo_engine::{clock::SessionClock, sources::{SourceContext, simulated::{Control, SimulatedContext, motions::Motion}}};
#[test]
fn simulator_controls_extend_a_source_independent_context() {
    let (events,_)=tokio::sync::mpsc::channel(1);
    let common=SourceContext {events,clock:SessionClock::start(),cancel:tokio_util::sync::CancellationToken::new(),dropped:Default::default()};
    let (_,control)=tokio::sync::watch::channel(Control{motion:Motion::Standing,speed:1.0,paused:false,generation:0,suspended:false});
    let (_,faults)=tokio::sync::watch::channel(Default::default());
    let simulator=SimulatedContext{common:common.clone(),control,faults};
    assert!(!simulator.as_ref().cancel.is_cancelled());
    assert_eq!(simulator.control.borrow().motion,Motion::Standing);
}
