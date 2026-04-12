use domain::events::DomainEvent;
use tokio::sync::broadcast;

pub type EventSender = broadcast::Sender<DomainEvent>;
pub type EventReceiver = broadcast::Receiver<DomainEvent>;

pub fn new_bus(capacity: usize) -> EventSender {
    broadcast::channel(capacity).0
}
