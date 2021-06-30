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
    if let Some(state) = state_from_str(input) {
        match state {
            State::Pending(m) => {
                m.submit();
            },
            _ => println!("irrelevant state")
        }
    }
}