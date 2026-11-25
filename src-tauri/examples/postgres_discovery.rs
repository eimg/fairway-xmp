//! Read-only tool inventory; --inspect explicitly authorizes --version only.
use fairway_core::postgresql::{Discovery, InspectRequest};
fn main() {
    let inspect = match std::env::args().nth(1).as_deref() {
        None => false,
        Some("--inspect") if std::env::args().len() == 2 => true,
        _ => {
            eprintln!("Usage: postgres_discovery [--inspect]");
            std::process::exit(2);
        }
    };
    let service = Discovery::new(None);
    let inventory = service.discover();
    if inspect {
        for candidate in inventory.candidates {
            match service.inspect(InspectRequest {
                candidate_id: candidate.id,
                confirmed: true,
            }) {
                Ok(result) => println!("{}", serde_json::to_string(&result).unwrap()),
                Err(error) => {
                    eprintln!("{}", serde_json::to_string(&error).unwrap());
                    std::process::exit(1);
                }
            }
        }
    } else {
        println!("{}", serde_json::to_string(&inventory).unwrap());
    }
}
