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
    #[test]
    fn test_updater() {
        use super::DeadlineUpdater;
        use std::time::{Duration, SystemTime};

        let mut counter = 0;
        let mut updater = DeadlineUpdater::new(|| {
            counter += 1;
            let next_deadline = SystemTime::now() + Duration::from_secs(2);
            (next_deadline, counter)
        });

        // Initial get should return 1
        let value1 = updater.get().unwrap();
        assert_eq!(value1, 1);

        // Immediate second get should return cached value 1
        let value2 = updater.get().unwrap();
        assert_eq!(value2, 1);

        // Wait for more than 2 seconds to exceed deadline
        std::thread::sleep(Duration::from_secs(3));

        // Next get should refresh and return 2
        let value3 = updater.get().unwrap();
        assert_eq!(value3, 2);
    }
}
