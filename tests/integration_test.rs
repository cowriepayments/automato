use serde::{ Serialize, Deserialize };
use serde_json;
use automato::statemachine;

#[derive(Serialize, Deserialize, Clone, Copy)]
struct TxData {}

#[derive(Serialize, Deserialize, Clone, Copy)]
struct AssociatedData {}

struct Log {}
impl Observer for Log {
    fn on_init<T:Serialize, U:Serialize>(&self, to: &str, data:Option<T> , state_data:Option<U>) ->Result<(),()> {
        println!("initializing to {}", to);

        if let Some(d) = data {
            match serde_json::to_string(&d) {
                Ok(s) => println!("{}", s),
                Err(_) => return Err(())
            };
        };

        if let Some(d) = state_data {
            match serde_json::to_string(&d) {
                Ok(s) => println!("{}", s),
                Err(_) => return Err(())
            };
        };

        Ok(())
    }
}

statemachine! {
    Tx: TxData {
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
    let tx = Tx::init(TxData {}, AssociatedData {}, Log {}).unwrap();
    let tx = tx.submit(AssociatedData{}).unwrap();
    tx.accept(AssociatedData{}).unwrap();

    let shared_data = TxData {};
    let state_data = AssociatedData {};
    let json_shared_data = serde_json::to_string(&shared_data).unwrap();
    let json_state_data = serde_json::to_string(&state_data).unwrap();
    let bx = restore("Pending", Some(Encoded::Json(json_shared_data)), Some(Encoded::Json(json_state_data)), Log {});
    if let State::Pending(cx) = bx.unwrap() {
        cx.submit(AssociatedData{}).unwrap();
    };
}