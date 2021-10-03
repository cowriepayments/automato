use async_trait::async_trait;
use automato::statemachine;
use futures::executor::block_on;
use serde::{Deserialize, Serialize};
use serde_json;
use std::marker::PhantomData;

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
impl Observer<()> for Log {
    type Error = ();
}

#[test]
fn init() {
    let _job = block_on(Job::init(
        &mut (),
        Log {},
        Some("foo".to_string()),
        JobData {},
        QueuedData {},
    ))
    .unwrap();
}

#[test]
fn init_without_id() {
    let result = block_on(Job::init(&mut (), Log {}, None, JobData {}, QueuedData {}));
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
    impl Observer<()> for DeferredIdInitLog {
        type Error = ();

        async fn on_init<'a>(
            &mut self,
            _ctx: &mut (),
            _to: State<'a>,
            _id: Option<String>,
            _data: &JobData,
        ) -> Result<Option<String>, Self::Error> {
            Ok(Some("foo".to_string()))
        }
    }

    let _job = block_on(Job::init(
        &mut (),
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
        initiated_to_state: Option<String>,
    }

    #[async_trait]
    impl Observer<()> for &mut InitLog {
        type Error = ();

        async fn on_init<'a>(
            &mut self,
            _ctx: &mut (),
            to: State<'a>,
            id: Option<String>,
            _data: &JobData,
        ) -> Result<Option<String>, Self::Error> {
            self.initiated_to_state = Some(to.to_string());
            Ok(id)
        }
    }

    let mut init_log = InitLog {
        initiated_to_state: None,
    };

    let _job = block_on(Job::init(
        &mut (),
        &mut init_log,
        Some("foo".to_string()),
        JobData {},
        QueuedData {},
    ))
    .unwrap();

    match init_log.initiated_to_state {
        Some(state) => assert_eq!("Queued", state),
        None => panic!("expected some initiated_to_state value"),
    };
}

#[test]
fn read_id() {
    let job = block_on(Job::init(
        &mut (),
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
        &mut (),
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
        &mut (),
        Log {},
        Some("foo".to_string()),
        JobData {},
        QueuedData {},
    ))
    .unwrap();
    let _job = block_on(job.start(&mut (), ProcessingData {})).unwrap();
}

#[test]
fn on_transition() {
    struct TransitionLog {
        from: Option<String>,
        to: Option<String>,
    }

    #[async_trait]
    impl Observer<()> for &mut TransitionLog {
        type Error = ();

        async fn on_transition<'a>(
            &mut self,
            _ctx: &mut (),
            from: State<'a>,
            to: State<'a>,
            _id: &str,
            _data: &JobData,
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

    let job = block_on(Job::init(
        &mut (),
        &mut transition_log,
        Some("foo".to_string()),
        JobData {},
        QueuedData {},
    ))
    .unwrap();
    let _job = block_on(job.start(&mut (), ProcessingData {})).unwrap();

    match transition_log.from {
        Some(state) => assert_eq!("Queued", state),
        None => panic!("expected some from value"),
    };

    match transition_log.to {
        Some(state) => assert_eq!("Processing", state),
        None => panic!("expected some to value"),
    };
}
