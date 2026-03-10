use std::sync::Mutex;
use std::time::{Duration, SystemTime};

#[allow(dead_code)]
pub struct DeadlineUpdater<T, F>
where
    F: FnMut() -> (SystemTime, T),
{
    pub inner: Mutex<DeadlineUpdaterInner<T>>,
    pub update_fn: F,
}

pub struct DeadlineUpdaterInner<T> {
    pub value: Option<T>,
    pub deadline: SystemTime,
}

impl<T, F> DeadlineUpdater<T, F>
where
    T: Clone,
    F: FnMut() -> (SystemTime, T),
{
    pub fn new(update_fn: F) -> Self {
        Self {
            inner: Mutex::new(DeadlineUpdaterInner {
                value: None,
                // Start "already expired" (1 minute in the past) to force initial refresh
                deadline: SystemTime::now()
                    .checked_sub(Duration::from_secs(60))
                    .unwrap_or(SystemTime::UNIX_EPOCH),
            }),
            update_fn,
        }
    }

    pub fn get(&mut self) -> Result<T, String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("Mutex poisoned: {}", e))?;

        let now = SystemTime::now();
        let needs_refresh = guard.value.is_none() || now >= guard.deadline;

        if needs_refresh {
            let (next_deadline, new_value) = (self.update_fn)();
            guard.deadline = next_deadline;
            guard.value = Some(new_value);
        }

        match guard.value {
            Some(ref v) => Ok(v.clone()),
            None => Err("No value after update".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DeadlineUpdater;
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_updater_initial_refresh() {
        let mut counter = 0;
        let mut updater = DeadlineUpdater::new(|| {
            counter += 1;
            let next_deadline = SystemTime::now() + Duration::from_secs(60);
            (next_deadline, counter)
        });

        // Initial get should return 1 (triggers refresh because starts expired)
        let value1 = updater.get().unwrap();
        assert_eq!(value1, 1);

        // Immediate second get should return cached value 1
        let value2 = updater.get().unwrap();
        assert_eq!(value2, 1);
    }

    #[test]
    fn test_updater_expired_deadline() {
        let mut counter = 0;
        let mut updater = DeadlineUpdater::new(|| {
            counter += 1;
            // Set deadline in the past to force refresh on next get
            let next_deadline = SystemTime::now()
                .checked_sub(Duration::from_secs(1))
                .unwrap_or(SystemTime::UNIX_EPOCH);
            (next_deadline, counter)
        });

        // First get triggers refresh
        let value1 = updater.get().unwrap();
        assert_eq!(value1, 1);

        // Second get also triggers refresh because deadline is in the past
        let value2 = updater.get().unwrap();
        assert_eq!(value2, 2);

        // Third get also triggers refresh
        let value3 = updater.get().unwrap();
        assert_eq!(value3, 3);
    }
}
