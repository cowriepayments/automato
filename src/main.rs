mod tx {
    use statemachine::statemachine;
    use sm::{ Machine, State, Pending, state_from_str };
    
    statemachine! {
        sm {
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

    pub struct Tx {
        state: State
    }

    impl Tx {
        pub fn new() -> Self {
            Tx {
                state: State::Pending(Machine::<Pending>::new())
            }
        }

        pub fn restore_from_state(raw_state: &str) -> Result<Self, ()> {
            if let Some(state) = state_from_str(raw_state) {
                return Ok(Tx {
                    state
                })
            }

            Err(())
        }

        pub fn submit(&mut self) {
            self.state = match self.state {
                State::Pending(m) => {
                    // do some work to submit the tranaction

                    m.submit()
                },
                _ => self.state
            }
        }

        pub fn accept(&mut self) {
            self.state = match self.state {
                State::Submitting(m) => m.accept(),
                State::Submitted(m) => m.accept(),
                _ => self.state
            };
        }
    }
}

fn main() {
    use tx::Tx;
    
    let mut tx = Tx::new();
    tx.submit();
    tx.accept();

    if let Ok(mut mx) = Tx::restore_from_state("Submitted") {
        mx.submit();
        mx.accept();

    }
}