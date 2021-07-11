use serde::{ Serialize, Deserialize };
use serde_json;
use automato::statemachine;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct JobData {}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct QueuedData {}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct ProcessingData {}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct CompletedData {}

statemachine! {
    Job: JobData {
        init Queued: QueuedData {
            start => Processing
        },
        Processing: ProcessingData {
            complete => Completed,
            queue => Queued
        },
        Completed: CompletedData {}
    }
}

struct Log {}
impl Observer for Log {
    type Error = ();
}

#[test]
fn init() {
    let _job = Job::init(JobData {}, QueuedData {}, Log {}).unwrap();
}

#[test]
fn on_init() {
    struct InitLog {
        initiated_to_state: Option<State>
    }
    
    impl Observer for &mut InitLog {
        type Error = ();

        fn on_init<T:Serialize,U:Serialize>(&mut self, to: State, _data:Option<T> , _state_data:Option<U>) ->Result<(),Self::Error> {
            self.initiated_to_state = Some(to);
            Ok(())
        }
    }

    let mut init_log = InitLog {
        initiated_to_state: None
    };

    let _job = Job::init(JobData {}, QueuedData {}, &mut init_log).unwrap();

    match init_log.initiated_to_state {
        Some(state) => assert_eq!("Queued", state.to_string()),
        None => panic!("expected some initiated_to_state value")
    };
}

#[test]
fn read_data() {
    let job = Job::init(JobData {}, QueuedData {}, Log {}).unwrap();
    let _job_data = job.data();
    let _job_state_data = job.state.data();
}

#[test]
fn transition() {
    let job = Job::init(JobData {}, QueuedData {}, Log {}).unwrap();
    let _job = job.start(ProcessingData {}).unwrap();
}

#[test]
fn on_transition() {
    struct TransitionLog {
        from: Option<State>,
        to: Option<State>
    }

    impl Observer for &mut TransitionLog {
        type Error = ();

        fn on_transition<T:Serialize>(&mut self, from: State, to: State, _data:Option<T>) ->Result<(),Self::Error> {
            self.from = Some(from);
            self.to = Some(to);
            Ok(())
        }
    }

    let mut transition_log = TransitionLog {
        from: None,
        to: None
    };

    let job = Job::init(JobData {}, QueuedData {}, &mut transition_log).unwrap();
    let _job = job.start(ProcessingData {}).unwrap();

    match transition_log.from {
        Some(state) => assert_eq!("Queued", state.to_string()),
        None => panic!("expected some from value")
    };

    match transition_log.to {
        Some(state) => assert_eq!("Processing", state.to_string()),
        None => panic!("expected some to value")
    };
}