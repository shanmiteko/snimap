mod client;
mod myssl_com;
mod nexcess_net;

pub use myssl_com::MysslCom;
pub use nexcess_net::NexcessNet;

#[cfg(test)]
mod tests {
    use crate::resolver::Lookup;

    use super::{MysslCom, NexcessNet};

    #[tokio::test]
    async fn nexcess_net() {
        assert!(MysslCom::default().lookup("wikipedia.org").await.is_some())
    }

    #[tokio::test]
    async fn myssl_com() {
        assert!(NexcessNet::default()
            .lookup("wikipedia.org")
            .await
            .is_some())
    }
}
