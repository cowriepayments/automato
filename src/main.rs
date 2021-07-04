use automato::statemachine;

statemachine! {
    {
        Pending {
            submit Message => Submitting,
            cancel => Cancelled
        },
        Submitting {
            await_submit => Submitted,
            accept => Accepted,
            decline => Declined
        },
        Submitted {
            accept => Accepted,
            decline => Declined
        },
        Accepted {},
        Declined {},
        Cancelled {}
    }
}

struct Message {}
impl EventData for Message {
    type Data = Message;
    fn json_encode(&self) -> String {
        "".to_string()
    }
    fn json_decode(json: &str) -> Result<Self::Data, ()> {
        println!("{}", json);
        Ok(Message {})
    }
}

struct Tx {}

impl OnChangeState for Tx {
    fn on_change_state<T: EventData>(&self, from: &str, to: &str, data: Option<T>) ->Result<(),()>  {
        println!("transitioning from {} to {}", from, to);
        if let Some(d) = data {
            println!("{}", d.json_encode());
        }

        Ok(())
    }
}

impl Tx {
    fn wrapped() -> State<Tx> {
        State::Pending(Machine::<Pending, Tx>::new(Tx {}))
    }
}

fn main() {
    let tx = Tx::wrapped();
    let tx = match tx {
        State::Pending(m) => m.submit(Message {}).unwrap(),
        _ => tx
    };

    if let State::Submitting(m) = tx {
        m.await_submit().unwrap();
    }
}