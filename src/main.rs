use statemachine::statemachine;

statemachine! {
    transaction {
        Pending {
            submit => Submitting,
            cancel => Cancelled
        },
        Submitting {
            await_submit_result => Submitted,
            accept => Accepted,
            decline => Declined
        },
        Submitted {
            accept => Accepted,
            decline => Declined
        },
        Accepted {
            decline => Declined
        },
        Declined {},
        Cancelled {}
    }
}

fn main() {
    use transaction::{ State, state_from_str };

    let input = "Pending";
    match state_from_str(input) {
        State::Pending(m) => {
            m.submit();
        },
        State::Submitting(m) => {
            m.await_submit_result();
        },
        State::Submitted(m) => {
            m.accept();
        },
        _ => {
            // no-op
        }
    }
}