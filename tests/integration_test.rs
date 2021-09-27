use async_trait::async_trait;
use automato::statemachine;
use futures::executor::block_on;
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

#[async_trait]
impl Observer for Log {
    type Error = ();
    type Context = ();
}

#[test]
fn init() {
    let _job = block_on(Job::init(
        (),
        Log {},
        Some("foo".to_string()),
        JobData {},
        QueuedData {},
    ))
    .unwrap();
}

#[test]
fn init_without_id() {
    let result = block_on(Job::init((), Log {}, None, JobData {}, QueuedData {}));
    let err = result.err().unwrap();
    match err {
        InitError::EmptyId => {}
        _ => panic!("expected InitErr::EmptyId"),
    };
}

#[test]
fn init_with_deferred_id() {
    struct DeferredIdInitLog {}

    #[async_trait]
    impl Observer for DeferredIdInitLog {
        type Error = ();
        type Context = ();

        async fn on_init<T: Serialize + Send, U: Serialize + Send>(
            &mut self,
            _ctx: &mut Self::Context,
            _id: Option<String>,
            _to: State,
            _data: Option<T>,
            _state_data: Option<U>,
        ) -> Result<Option<String>, Self::Error> {
            Ok(Some("foo".to_string()))
        }
    }

    let _job = block_on(Job::init(
        (),
        DeferredIdInitLog {},
        None,
        JobData {},
        QueuedData {},
    ))
    .unwrap();
}

#[test]
fn on_init() {
    struct InitLog {
        initiated_to_state: Option<State>,
    }

    #[async_trait]
    impl Observer for &mut InitLog {
        type Error = ();
        type Context = ();

        async fn on_init<T: Serialize + Send, U: Serialize + Send>(
            &mut self,
            _ctx: &mut Self::Context,
            id: Option<String>,
            to: State,
            _data: Option<T>,
            _state_data: Option<U>,
        ) -> Result<Option<String>, Self::Error> {
            self.initiated_to_state = Some(to);
            Ok(id)
        }
    }

    let mut init_log = InitLog {
        initiated_to_state: None,
    };

    let _job = block_on(Job::init(
        (),
        &mut init_log,
        Some("foo".to_string()),
        JobData {},
        QueuedData {},
    ))
    .unwrap();

    match init_log.initiated_to_state {
        Some(state) => assert_eq!("Queued", state.to_string()),
        None => panic!("expected some initiated_to_state value"),
    };
}

#[test]
fn read_id() {
    let job = block_on(Job::init(
        (),
        Log {},
        Some("foo".to_string()),
        JobData {},
        QueuedData {},
    ))
    .unwrap();
    let id = job.id();

    assert_eq!(id, "foo");
}

#[test]
fn read_data() {
    let job = block_on(Job::init(
        (),
        Log {},
        Some("foo".to_string()),
        JobData {},
        QueuedData {},
    ))
    .unwrap();
    let _job_data = job.data();
    let _job_state_data = job.state.data();
}

#[test]
fn transition() {
    let job = block_on(Job::init(
        (),
        Log {},
        Some("foo".to_string()),
        JobData {},
        QueuedData {},
    ))
    .unwrap();
    let _job = block_on(job.start((), ProcessingData {})).unwrap();
}

#[test]
fn on_transition() {
    struct TransitionLog {
        from: Option<State>,
        to: Option<State>,
    }

    #[async_trait]
    impl Observer for &mut TransitionLog {
        type Error = ();
        type Context = ();

        async fn on_transition<T: Serialize + Send, U: Serialize + Send>(
            &mut self,
            _ctx: &mut Self::Context,
            _id: &str,
            from: State,
            to: State,
            _data: Option<T>,
            _state_data: Option<U>,
        ) -> Result<(), Self::Error> {
            self.from = Some(from);
            self.to = Some(to);
            Ok(())
        }
    }

    let mut transition_log = TransitionLog {
        from: None,
        to: None,
    };

    let job = block_on(Job::init(
        (),
        &mut transition_log,
        Some("foo".to_string()),
        JobData {},
        QueuedData {},
    ))
    .unwrap();
    let _job = block_on(job.start((), ProcessingData {})).unwrap();

    match transition_log.from {
        Some(state) => assert_eq!("Queued", state.to_string()),
        None => panic!("expected some from value"),
    };

    match transition_log.to {
        Some(state) => assert_eq!("Processing", state.to_string()),
        None => panic!("expected some to value"),
    };
}
