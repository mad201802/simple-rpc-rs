use rand::Rng;

pub async fn generate_random_request_id() -> u8 {
    let mut rng = rand::thread_rng();
    rng.gen::<u8>()
}
