pub mod spider {
    use wreq::Client;
    use wreq_util::Emulation;

    pub fn get(url: &str, profile: Option<&str>) -> String {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let emu = match profile.unwrap_or("chrome") {
                "safari" => Emulation::Safari26,
                _ => Emulation::Chrome124,
            };

            let client = Client::builder()
                .emulation(emu)
                .build()
                .unwrap();

            client.get(url).send().await.unwrap().text().await.unwrap()
        })
    }
}


