use automato_sync::statemachine;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json;

#[derive(Serialize, Deserialize)]
pub struct JobData {}

#[derive(Serialize, Deserialize)]
pub struct QueuedData {}

#[derive(Serialize, Deserialize)]
pub struct ProcessingData {}

#[derive(Serialize, Deserialize)]
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

impl Observer<()> for Log {
    type Data = ();
    type QueuedData = ();
    type ProcessingData = ();
    type CompletedData = ();
    type Error = ();
}

#[test]
fn init() {
    let _job = Job::init(&mut (), Log {}, Some("foo".to_string()), (), ()).unwrap();
}

#[test]
fn init_without_id() {
    let result = Job::init(&mut (), Log {}, None, (), ());
    let err = result.err().unwrap();
    match err {
        InitError::EmptyId => {}
        _ => panic!("expected InitErr::EmptyId"),
    };
}

#[test]
fn init_with_deferred_id() {
    struct DeferredIdInitLog {}

    impl Observer<()> for DeferredIdInitLog {
        type Data = JobData;
        type QueuedData = ();
        type ProcessingData = ();
        type CompletedData = ();
        type Error = ();

        fn on_init<'a>(
            &mut self,
            _ctx: &mut (),
            _id: Option<String>,
            to: State<'a, (), Self>,
            _data: &Self::Data,
        ) -> Result<Option<String>, Self::Error> {
            match to {
                State::Queued(_) => (),
                _ => panic!("unexpected initial state"),
            };
            Ok(Some("foo".to_string()))
        }
    }

    let _job = Job::init(&mut (), DeferredIdInitLog {}, None, JobData {}, ()).unwrap();
}

#[test]
fn on_init() {
    struct InitLog {
        initiated_to_state: Option<String>,
    }

    impl Observer<()> for &mut InitLog {
        type Data = ();
        type QueuedData = ();
        type ProcessingData = ();
        type CompletedData = ();
        type Error = ();

        fn on_init<'a>(
            &mut self,
            _ctx: &mut (),
            id: Option<String>,
            to: State<'a, (), Self>,
            _data: &Self::Data,
        ) -> Result<Option<String>, Self::Error> {
            self.initiated_to_state = Some(to.to_string());
            Ok(id)
        }
    }

    let mut init_log = InitLog {
        initiated_to_state: None,
    };

    let _job = Job::init(&mut (), &mut init_log, Some("foo".to_string()), (), ()).unwrap();

    match init_log.initiated_to_state {
        Some(state) => assert_eq!("Queued", state),
        None => panic!("expected some initiated_to_state value"),
    };
}

#[test]
fn read_id() {
    let job = Job::init(&mut (), Log {}, Some("foo".to_string()), (), ()).unwrap();
    let id = job.id();

    assert_eq!(id, "foo");
}

#[test]
fn read_data() {
    let job = Job::init(&mut (), Log {}, Some("foo".to_string()), (), ()).unwrap();
    let _job_data = job.data();
    let _job_state_data = job.state.data();
}

#[test]
fn transition() {
    let job = Job::init(&mut (), Log {}, Some("foo".to_string()), (), ()).unwrap();
    let _job = job.start(&mut (), ()).unwrap();
}

#[test]
fn on_transition() {
    struct TransitionLog {
        from: Option<String>,
        to: Option<String>,
    }

    impl Observer<()> for &mut TransitionLog {
        type Data = ();
        type QueuedData = ();
        type ProcessingData = ();
        type CompletedData = ();
        type Error = ();

        fn on_transition<'a>(
            &mut self,
            _ctx: &mut (),
            _id: &str,
            from: State<'a, (), Self>,
            to: State<'a, (), Self>,
            _data: &Self::Data,
        ) -> Result<(), Self::Error> {
            self.from = Some(from.to_string());
            self.to = Some(to.to_string());
            Ok(())
        }
    }

    let mut transition_log = TransitionLog {
        from: None,
        to: None,
    };

    let job = Job::init(
        &mut (),
        &mut transition_log,
        Some("foo".to_string()),
        (),
        (),
    )
    .unwrap();
    let _job = job.start(&mut (), ()).unwrap();

    match transition_log.from {
        Some(state) => assert_eq!("Queued", state),
        None => panic!("expected some from value"),
    };

    match transition_log.to {
        Some(state) => assert_eq!("Processing", state),
        None => panic!("expected some to value"),
    };
}
