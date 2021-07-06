use automato::statemachine;
struct SharedData {}

struct AssociatedData {}

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

fn main() {
    let tx: Tx<Pending> = Tx::init(SharedData {}, AssociatedData {});
    let tx = tx.submit(AssociatedData{});
    let tx= tx.accept(AssociatedData{});
}