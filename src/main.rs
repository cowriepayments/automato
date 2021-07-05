use automato::statemachine;

struct Tx {}
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
}