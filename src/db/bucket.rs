use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use log::{error, info};

#[derive(Debug)]
pub struct DatabaseBucket {
    connection: Arc<cassandra_cpp::Session>,
    reference_count: Arc<AtomicUsize>,
}

impl From<Arc<cassandra_cpp::Session>> for DatabaseBucket {
    fn from(value: Arc<cassandra_cpp::Session>) -> Self {
        let count = Arc::strong_count(&value);
        Self {
            connection: value,
            reference_count: Arc::new(AtomicUsize::from(count)),
        }
    }
}

impl Clone for DatabaseBucket {
    fn clone(&self) -> Self {
        Self {
            connection: self.connection.clone(),
            reference_count: self.reference_count.clone(),
        }
    }
}

impl DatabaseBucket {
    pub async fn new() -> DatabaseBucket {
        let contact_points = std::env::var("CASSANDRA_HOST").unwrap_or(String::from("127.0.0.1"));

        let mut cluster = cassandra_cpp::Cluster::default();
        let cluster = cluster
            .set_contact_points(contact_points.as_str())
            .expect("Failed to set contact points");
        cluster.set_load_balance_round_robin();

        while let Err(err) = cluster.connect().await {
            error!("{}", err);
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        let session = cluster.connect().await.unwrap();

        Self {
            connection: Arc::new(session),
            reference_count: Arc::new(1.into()),
        }
    }

    /// Clones the connection and increments reference count
    ///
    /// Reference count in this situation is the count of the
    /// users that are connected to the server,
    /// so whenever some new connection clones `DatabaseBucket`, the
    /// internal reference count is the same
    pub fn clone_increment_rc(&self) -> Self {
        let rc = Arc::clone(&self.reference_count);
        rc.fetch_add(1, Ordering::SeqCst);
        self.clone()
    }

    pub fn decrement_rc(&mut self) {
        let rc = Arc::clone(&self.reference_count);
        rc.fetch_sub(1, Ordering::SeqCst);
    }

    pub fn is_full(&self) -> bool {
        let current_value = self.reference_count.clone().load(Ordering::SeqCst);
        current_value >= 3
    }

    pub fn get_connection(&self) -> Arc<cassandra_cpp::Session> {
        self.connection.clone()
    }

    pub fn is_rc_zero(&self) -> bool {
        self.reference_count.load(Ordering::SeqCst) == 0
    }

    pub fn get_rc_count(&self) -> usize {
        self.reference_count.load(Ordering::SeqCst)
    }
}

pub struct DatabaseBuilder {
    pub bucket: DatabaseBucket,
}

impl From<DatabaseBucket> for DatabaseBuilder {
    fn from(value: DatabaseBucket) -> Self {
        Self { bucket: value }
    }
}

impl DatabaseBuilder {
    pub fn from_raw(connection: Arc<cassandra_cpp::Session>) -> Self {
        Self {
            bucket: connection.into(),
        }
    }
}

pub struct DatabasePool {
    buckets: Vec<DatabaseBucket>,
}

impl DatabasePool {
    pub async fn new() -> Self {
        let buckets: Vec<DatabaseBucket> = vec![DatabaseBucket::new().await];

        Self { buckets }
    }

    pub async fn get_available_bucket(&mut self) -> DatabaseBucket {
        info!(
            "Getting available db bucket...\n Buckets: {:?}",
            self.buckets
        );
        for (i, bucket) in self.buckets.iter_mut().enumerate() {
            if !bucket.is_full() {
                return bucket.clone_increment_rc();
            }

            //if bucket.is_rc_zero() {
            //    tokio::spawn({
            //        let i = i.clone();
            //        async move {
            //            // Wait for 120 secs and check if the rc is still 0...
            //            // Needed because statistically, it's more likely that new user is going to
            //            // join in theese 120 secs
            //            tokio::time::sleep(std::time::Duration::from_secs(120)).await;

            //            if self.buckets[i].is_rc_zero() {
            //                self.buckets.remove(i);
            //            }
            //        }
            //    });
            //}
        }

        // Sort by reference count in ascending order
        self.buckets
            .sort_by(|a, b| a.get_rc_count().cmp(&b.get_rc_count()));

        let new_bucket = DatabaseBucket::new().await;
        self.buckets.push(new_bucket.clone());
        info!(
            "Creating new Bucket in Database Pool!\nCurrent pool size: {}",
            self.buckets.len()
        );
        new_bucket
    }
}
