mod dow;
pub mod resolver;
mod spiders;

// https://76.76.2.0/p0
// https://public.dns.iij.jp/dns-query

#[cfg(test)]
mod tests {
    use once_cell::sync::Lazy;

    use crate::resolver::Resolver;
    static RESOLVER: Lazy<Resolver> = Lazy::new(Resolver::default);

    #[tokio::test]
    async fn resolver_test() {
        let resolver = Resolver::default();
        assert!(resolver.lookup_ip("wikipedia.org").await.is_some());
        assert!(resolver.lookup_ip("wikipedia.org").await.is_some());
        assert!(resolver.lookup_ip("wikipedia.org").await.is_some());
    }

    #[tokio::test]
    async fn resolver_test_muti() {
        tokio::spawn(async { assert!(RESOLVER.lookup_ip("wikipedia.org").await.is_some()) })
            .await
            .unwrap();
    }
}
