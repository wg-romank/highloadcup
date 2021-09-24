use tokio::sync::mpsc;

pub trait Actor {
    fn start(self);
}

#[derive(Clone)]
pub struct Handler<Message> {
    pub tx: mpsc::Sender<Message>,
}

impl<M> Handler<M> {
    pub fn new<A: Actor, MA>(make_actor: MA) -> Handler<M>
    where
        MA: FnOnce(mpsc::Receiver<M>) -> A,
    {
        let (tx, rx) = mpsc::channel(1000);
        make_actor(rx).start();
        Self { tx }
    }
}
