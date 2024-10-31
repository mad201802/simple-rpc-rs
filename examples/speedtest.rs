use std::{
    io::{Read, Write},
    net::TcpStream,
    thread,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to the server
    let mut stream = TcpStream::connect("127.0.0.1:6969")?;
    println!("Connected to the server!");

    let message = b"Hello\n";
    let message_size = message.len();
    let num_rounds = 100;

    // Continuously run the speed test every 5 seconds
    loop {
        let mut total_duration = Duration::from_secs(0);

        // Perform multiple speed tests (round-trips)
        for _ in 0..num_rounds {
            // Measure the time it takes to send and receive the message
            let start = Instant::now();

            // Send the message to the server
            stream.write_all(message)?;

            // Read the response from the server
            let mut buf = vec![0u8; message_size];
            stream.read_exact(&mut buf)?;

            let duration = start.elapsed();
            total_duration += duration;

            println!(
                "Round trip time: {:?} for message: {:?}",
                duration,
                String::from_utf8_lossy(&buf)
            );
        }

        // Calculate the average round-trip time
        let avg_duration = total_duration / num_rounds as u32;
        println!(
            "Average round-trip time over {} rounds: {:?}",
            num_rounds, avg_duration
        );

        // Wait for 5 seconds before repeating the test
        thread::sleep(Duration::from_secs(1));
    }
}
