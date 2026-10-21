//! The CPU gate: a fixed number of slots handed out round-robin by user instead of first come, first served (WP7 review M2). A user
//! with ninety queued files takes one slot per turn like everyone else, so a later user waits for at most one slot per other
//! waiting user. When nobody else waits, one user may still use every slot.
//!
//! Waiting is cancel-safe: a dropped `acquire` future leaves a dead entry that the next release skips, and a slot that was already
//! handed to it comes back through `Permit::drop`.
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

#[derive(Default)]
struct Inner {
    free: usize,
    queues: HashMap<String, VecDeque<oneshot::Sender<Permit>>>,
    /// users with a non-empty queue, in turn order
    order: VecDeque<String>,
}

#[derive(Clone)]
pub struct Gate(Arc<Mutex<Inner>>);

/// A held slot; it goes to the next waiting user (or back to the pool) when dropped.
pub struct Permit(Option<Gate>);

impl Drop for Permit {
    fn drop(&mut self) {
        if let Some(g) = self.0.take() {
            g.release();
        }
    }
}

impl Gate {
    pub fn new(slots: usize) -> Gate {
        Gate(Arc::new(Mutex::new(Inner { free: slots, ..Inner::default() })))
    }

    /// A slot if one is free right now.
    pub fn try_acquire(&self) -> Option<Permit> {
        let mut g = self.0.lock().unwrap();
        (g.free > 0).then(|| {
            g.free -= 1;
            Permit(Some(self.clone()))
        })
    }

    pub async fn acquire(&self, user: &str) -> Permit {
        let rx = {
            let mut g = self.0.lock().unwrap();
            if g.free > 0 {
                g.free -= 1;
                return Permit(Some(self.clone()));
            }
            let (tx, rx) = oneshot::channel();
            let q = g.queues.entry(user.to_string()).or_default();
            let was_empty = q.is_empty();
            q.push_back(tx);
            if was_empty && !g.order.iter().any(|u| u == user) {
                g.order.push_back(user.to_string());
            }
            rx
        };
        // the sender lives in the queue until it is used, and `release` always sends before it drops one
        rx.await.expect("the gate keeps its waiters")
    }

    fn release(&self) {
        let mut g = self.0.lock().unwrap();
        while let Some(user) = g.order.pop_front() {
            let Some(q) = g.queues.get_mut(&user) else { continue };
            let Some(tx) = q.pop_front() else {
                g.queues.remove(&user);
                continue;
            };
            if q.is_empty() {
                g.queues.remove(&user);
            } else {
                g.order.push_back(user);
            }
            // a waiter that gave up is skipped; its undelivered permit must not release again while the lock is held
            if let Err(mut p) = tx.send(Permit(Some(self.clone()))) {
                p.0 = None;
                continue;
            }
            return;
        }
        g.free += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn slots_go_round_robin_across_users_and_a_dropped_waiter_costs_nothing() {
        let gate = Gate::new(1);
        let held = gate.try_acquire().unwrap();
        assert!(gate.try_acquire().is_none());
        let order = Arc::new(Mutex::new(Vec::<String>::new()));
        let mut tasks = vec![];
        // alice queues three files before bob queues one: FIFO would serve bob last
        for (user, tag) in [("alice", "a1"), ("alice", "a2"), ("alice", "a3"), ("bob", "b1")] {
            let (g, o) = (gate.clone(), order.clone());
            tasks.push(tokio::spawn(async move {
                let p = g.acquire(user).await;
                o.lock().unwrap().push(tag.to_string());
                tokio::time::sleep(Duration::from_millis(20)).await;
                drop(p);
            }));
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        // a waiter that is dropped while queued must not eat a slot
        let g = gate.clone();
        let ghost = tokio::spawn(async move { drop(g.acquire("carol").await) });
        tokio::time::sleep(Duration::from_millis(20)).await;
        ghost.abort();
        drop(held);
        for t in tasks {
            t.await.unwrap();
        }
        assert_eq!(*order.lock().unwrap(), ["a1", "b1", "a2", "a3"]);
        assert!(gate.try_acquire().is_some(), "every slot came back");
    }
}
