use serde::Serialize;
use automato::statemachine;

#[derive(Serialize, Clone, Copy)]
struct SharedData {}

#[derive(Serialize, Clone, Copy)]
struct AssociatedData {}

struct Log {}
impl Observer for Log {}

statemachine! {
    Tx: SharedData {
        init Pending: AssociatedData {
            submit => Submitting,
            cancel => Cancelled
        },
        Submitting: AssociatedData {
            accept => Accepted,
            decline => Declined,
            await_submit => Submitted,
        },
        Submitted: AssociatedData {
            accept => Accepted,
            decline => Declined,
        },
        Accepted: AssociatedData {},
        Cancelled: AssociatedData {},
        Declined: AssociatedData {}
    }
}

#[test]
fn transitions() {
    let tx = Tx::init(SharedData {}, AssociatedData {}, Log {}).unwrap();
    let tx = tx.submit(AssociatedData{}).unwrap();
    tx.accept(AssociatedData{}).unwrap();
}