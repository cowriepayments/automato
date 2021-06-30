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
    use transaction::{ Machine, Pending };
    
    // initialize machiine in the Pending state
    let m = Machine::<Pending>::new();
    
    let m = m.submit();
    let m = m.await_submit_result();
    let m = m.accept();
    m.decline();
}